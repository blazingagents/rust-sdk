//! The browser relay: the one endpoint of your backend that
//! `useBlazingAgentsChat` and `BlazingAgentsChatTransport` from the TypeScript
//! SDK talk to. The browser holds no API key, so it posts every chat, history
//! and steer request to your backend, and [`ChatRelay`] forwards each one to
//! the API as the End-user your own sign-in found.
//!
//! The relay is framework neutral. [`ChatRelay::handle`] takes an
//! [`http::Request`] with any [`http_body::Body`] and returns an
//! [`http::Response`] whose [`RelayBody`] is both an [`http_body::Body`] and a
//! [`Stream`] of bytes, so axum, hyper and tower stacks
//! need no glue, and other frameworks wrap the stream.
//!
//! ```no_run
//! use axum::Router;
//! use axum::extract::{Request, State};
//! use axum::response::Response;
//! use axum::routing::post;
//! use blazing_agents::Client;
//! use blazing_agents::relay::{ChatRelay, RelayBody, RelayContext};
//!
//! async fn chat(State(relay): State<ChatRelay>, request: Request) -> Response<RelayBody> {
//!     relay.handle(request).await
//! }
//!
//! # type BoxError = Box<dyn std::error::Error + Send + Sync>;
//! # async fn verified_user(request: &http::Request<bytes::Bytes>) -> Result<Option<String>, BoxError> {
//! #     Ok(None)
//! # }
//! # async fn example() -> Result<(), BoxError> {
//! let relay = ChatRelay::new(Client::from_env()?, |request| async move {
//!     // Your own sign-in check, on the session cookie or token of the
//!     // request. `None` answers 401.
//!     let user_id = verified_user(&request).await?;
//!     Ok(user_id.map(|user_id| RelayContext::new("ag_4kP9sT2vXq7LmN3a", user_id)))
//! });
//! let app: Router = Router::new().route("/api/chat", post(chat)).with_state(relay);
//! # let _ = app;
//! # Ok(())
//! # }
//! ```
//!
//! A browser on another origin can read the `Location` header, which carries a
//! new Session's URL, only when your CORS layer exposes it.
//!
//! Documentation: <https://docs.blazingagents.com/api-reference/protocols/browser-relay>.

use std::error::Error as StdError;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use bytes::Bytes;
use futures_core::Stream;
use http::header::{CACHE_CONTROL, CONTENT_TYPE, LOCATION};
use http::{HeaderName, HeaderValue, Request, Response, StatusCode};
use http_body::{Body, Frame, SizeHint};
use http_body_util::{BodyExt, Limited};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::chat::{ChatStream, ToolApprovalDecision};
use crate::client::{Client, UserClient};
use crate::error::Error;
use crate::functions::{Functions, is_platform_id};
use crate::ids::{AgentId, SessionId};
use crate::messages::UserMessage;
use crate::pagination::Cursor;
use crate::types::Metadata;

/// The API's own request body limit, so the relay never reads a body the API
/// would refuse.
const MAX_BODY_BYTES: usize = 15 * 1024 * 1024;

/// The most history messages the API returns in one page.
const HISTORY_PAGE_SIZE: u8 = 200;

const X_BA_TURN_ID: HeaderName = HeaderName::from_static("x-ba-turn-id");
const X_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");

type ResolveFuture = Pin<
    Box<dyn Future<Output = Result<Option<RelayContext>, Box<dyn StdError + Send + Sync>>> + Send>,
>;

/// Who a request is for and which Agent serves it, as your sign-in found it.
/// Never build one from the request body.
#[derive(Clone, Debug)]
pub struct RelayContext {
    agent_id: AgentId,
    user_id: String,
    functions: Option<Functions>,
    metadata: Option<Metadata>,
}

impl RelayContext {
    /// The Agent that serves the request and the signed-in End-user. The Agent
    /// must belong to the End-user, with its Workspace, or the API answers
    /// `404`. `user_id` follows the [`Client::for_user`] rules.
    pub fn new(agent_id: impl Into<AgentId>, user_id: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            user_id: user_id.into(),
            functions: None,
            metadata: None,
        }
    }

    /// Backend functions for this request and for the continuation after a
    /// tool approval.
    #[must_use]
    pub fn functions(mut self, functions: Functions) -> Self {
        self.functions = Some(functions);
        self
    }

    /// Your own data, stored with each new Turn.
    #[must_use]
    pub fn metadata(mut self, metadata: Metadata) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// The endpoint behind `useBlazingAgentsChat`: it serves the browser's `chat`,
/// `history` and `steer` requests as the End-user that `resolve_context`
/// returns. Cloning is cheap.
///
/// Every API call goes through [`Client::for_user`], so the API answers `404`
/// for another End-user's Agent or Session. Errors answer
/// `{"error": {"code", "message"}}`: `400 invalid_request` for a bad body or an
/// unknown `op`, `401 unauthorized`, the API's own status and code, `502` when
/// the API cannot be reached or answers something the SDK cannot read, and
/// `500 internal_error` for anything else, never naming its cause. A client
/// that disconnects drops the response, which closes the API request and
/// cancels the Turn.
#[derive(Clone)]
pub struct ChatRelay {
    client: Client,
    resolve: Arc<dyn Fn(Request<Bytes>) -> ResolveFuture + Send + Sync>,
}

impl ChatRelay {
    /// A relay that sends every API request through `client`.
    ///
    /// `resolve_context` runs first on each request, with the request and its
    /// whole body. It returns the End-user's [`RelayContext`], `None` for a
    /// request nobody is signed in to (answered `401`), or an error when it
    /// cannot tell, such as a failed session lookup (answered `500`, so an
    /// outage does not sign users out).
    pub fn new<F, Fut>(client: Client, resolve_context: F) -> Self
    where
        F: Fn(Request<Bytes>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Option<RelayContext>, Box<dyn StdError + Send + Sync>>>
            + Send
            + 'static,
    {
        Self {
            client,
            resolve: Arc::new(move |request| Box::pin(resolve_context(request))),
        }
    }

    /// Answers one browser request. It never fails: every failure is the
    /// response. Dropping the future or the response body cancels the API
    /// request.
    pub async fn handle<B>(&self, request: Request<B>) -> Response<RelayBody>
    where
        B: Body + Send,
        B::Data: Send,
        B::Error: Into<Box<dyn StdError + Send + Sync>>,
    {
        let (parts, body) = request.into_parts();
        let Ok(body) = Limited::new(body, MAX_BODY_BYTES).collect().await else {
            return invalid("Invalid request body.");
        };
        let body = body.to_bytes();
        let context = match (self.resolve)(Request::from_parts(parts, body.clone())).await {
            Ok(Some(context)) => context,
            Ok(None) => return unauthorized(),
            Err(error) => return internal_error(&*error),
        };
        let user = match self.client.for_user(&context.user_id) {
            Ok(user) => user,
            Err(error) => return internal_error(&error),
        };
        let command = match serde_json::from_slice::<Wire>(&body)
            .map_err(|_| Invalid("Invalid request body."))
            .and_then(Wire::into_command)
        {
            Ok(command) => command,
            Err(Invalid(message)) => return invalid(message),
        };
        match command.run(&user, context).await {
            Ok(response) => response,
            Err(error) => failure(error),
        }
    }
}

impl fmt::Debug for ChatRelay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChatRelay")
            .field("client", &self.client)
            .finish_non_exhaustive()
    }
}

/// A relay response body: a JSON document, or the Turn's server-sent events as
/// they arrive. An error item is a stream that failed after the headers went
/// out, so the HTTP server aborts the connection and the browser reads the
/// stored transcript instead.
#[derive(Debug)]
pub struct RelayBody(Repr);

#[derive(Debug)]
enum Repr {
    Json(Option<Bytes>),
    Events(Box<ChatStream>),
}

impl Stream for RelayBody {
    type Item = Result<Bytes, Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match &mut self.get_mut().0 {
            Repr::Json(document) => Poll::Ready(document.take().map(Ok)),
            Repr::Events(events) => Pin::new(events).poll_next(cx),
        }
    }
}

impl Body for RelayBody {
    type Data = Bytes;
    type Error = Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Error>>> {
        self.poll_next(cx)
            .map(|item| item.map(|chunk| chunk.map(Frame::data)))
    }

    fn size_hint(&self) -> SizeHint {
        match &self.0 {
            Repr::Json(document) => {
                SizeHint::with_exact(document.as_ref().map_or(0, |bytes| bytes.len() as u64))
            }
            Repr::Events(_) => SizeHint::default(),
        }
    }
}

/// The request body, by `op`. Unknown keys are ignored, and a field that
/// breaks the contract fails the parse, so nothing reaches the API unchecked.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
enum Wire {
    Chat(ChatWire),
    History(HistoryWire),
    Steer(SteerWire),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatWire {
    message: Value,
    message_id: Option<NonEmpty>,
    session_id: Option<Session>,
    #[serde(default)]
    trigger: Trigger,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryWire {
    session_id: Session,
    cursor: Option<NonEmpty>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SteerWire {
    session_id: Session,
    request_id: RequestId,
    message: Value,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Trigger {
    #[default]
    SubmitMessage,
    RegenerateMessage,
}

/// `ss_` and 16 letters or digits.
#[derive(Deserialize)]
#[serde(try_from = "String")]
struct Session(SessionId);

impl TryFrom<String> for Session {
    type Error = &'static str;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if is_platform_id(&value, "ss_") {
            Ok(Self(SessionId::from(value)))
        } else {
            Err("a session ID is ss_ and 16 letters or digits")
        }
    }
}

#[derive(Deserialize)]
#[serde(try_from = "String")]
struct NonEmpty(String);

impl TryFrom<String> for NonEmpty {
    type Error = &'static str;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            Err("the value must not be empty")
        } else {
            Ok(Self(value))
        }
    }
}

/// 1 to 128 characters, and not a URL dot segment.
#[derive(Deserialize)]
#[serde(try_from = "String")]
struct RequestId(String);

impl TryFrom<String> for RequestId {
    type Error = &'static str;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let length = value.encode_utf16().count();
        if (1..=128).contains(&length) && value != "." && value != ".." {
            Ok(Self(value))
        } else {
            Err("a request ID is 1 to 128 characters and not . or ..")
        }
    }
}

/// A body that breaks the contract, with the message the browser gets.
struct Invalid(&'static str);

const BAD_MESSAGE: Invalid = Invalid("Invalid chat message.");

/// What a valid request asks for. Each variant holds exactly what its API
/// call needs, so an approval without a Session cannot be built.
enum Command {
    Start {
        message: UserMessage,
    },
    Resume {
        session: SessionId,
        message: UserMessage,
    },
    Regenerate {
        session: SessionId,
        message: UserMessage,
        message_id: Option<String>,
    },
    Approve {
        session: SessionId,
        decisions: Vec<ToolApprovalDecision>,
    },
    History {
        session: SessionId,
        cursor: Option<String>,
    },
    Steer {
        session: SessionId,
        request_id: String,
        message: UserMessage,
    },
}

impl Wire {
    fn into_command(self) -> Result<Command, Invalid> {
        match self {
            Wire::Chat(chat) => chat.into_command(),
            Wire::History(HistoryWire { session_id, cursor }) => Ok(Command::History {
                session: session_id.0,
                cursor: cursor.map(|cursor| cursor.0),
            }),
            Wire::Steer(SteerWire {
                session_id,
                request_id,
                message,
            }) => Ok(Command::Steer {
                session: session_id.0,
                request_id: request_id.0,
                message: serde_json::from_value(message)
                    .map_err(|_| Invalid("Steering requires a user message."))?,
            }),
        }
    }
}

impl ChatWire {
    fn into_command(self) -> Result<Command, Invalid> {
        let role = self.message.get("role").and_then(Value::as_str);
        match (role, self.session_id) {
            (Some("assistant"), Some(session)) => Ok(Command::Approve {
                session: session.0,
                decisions: approval_decisions(&self.message)?,
            }),
            (Some("assistant"), None) => {
                Err(Invalid("Tool approval requires an existing Session."))
            }
            (Some("user"), session) => {
                let message =
                    serde_json::from_value::<UserMessage>(self.message).map_err(|_| BAD_MESSAGE)?;
                Ok(match (session, self.trigger) {
                    (None, _) => Command::Start { message },
                    (Some(session), Trigger::SubmitMessage) => Command::Resume {
                        session: session.0,
                        message,
                    },
                    (Some(session), Trigger::RegenerateMessage) => Command::Regenerate {
                        session: session.0,
                        message,
                        message_id: self.message_id.map(|id| id.0),
                    },
                })
            }
            _ => Err(Invalid("Chat submission requires a user message.")),
        }
    }
}

#[derive(Deserialize)]
struct Approval {
    id: String,
    approved: bool,
    reason: Option<String>,
}

fn part_type(part: &Value) -> &str {
    part.get("type").and_then(Value::as_str).unwrap_or_default()
}

/// The decisions of the last assistant step: every tool part answered after
/// the last `step-start` part.
fn approval_decisions(message: &Value) -> Result<Vec<ToolApprovalDecision>, Invalid> {
    let parts = message
        .get("parts")
        .and_then(Value::as_array)
        .ok_or(BAD_MESSAGE)?;
    let last_step = parts
        .iter()
        .rposition(|part| part_type(part) == "step-start")
        .map_or(0, |index| index + 1);
    let decisions = parts[last_step..]
        .iter()
        .filter(|part| {
            (part_type(part).starts_with("tool-") || part_type(part) == "dynamic-tool")
                && part.get("state").and_then(Value::as_str) == Some("approval-responded")
        })
        .map(|part| {
            let approval = Approval::deserialize(part.get("approval").unwrap_or(&Value::Null))
                .map_err(|_| BAD_MESSAGE)?;
            let decision = if approval.approved {
                ToolApprovalDecision::approve(approval.id)
            } else {
                ToolApprovalDecision::deny(approval.id)
            };
            Ok(match approval.reason {
                Some(reason) => decision.reason(reason),
                None => decision,
            })
        })
        .collect::<Result<Vec<_>, Invalid>>()?;
    if decisions.is_empty() {
        return Err(Invalid("The message has no tool approval responses."));
    }
    Ok(decisions)
}

/// `metadata` and `functions` are setters of each turn's own call type, with no
/// trait in common.
macro_rules! attributed {
    ($call:expr, $metadata:expr, $functions:expr) => {{
        let call = $call;
        let call = match $metadata {
            Some(metadata) => call.metadata(metadata),
            None => call,
        };
        match $functions {
            Some(functions) => call.functions(functions),
            None => call,
        }
    }};
}

impl Command {
    async fn run(
        self,
        user: &UserClient,
        context: RelayContext,
    ) -> Result<Response<RelayBody>, Error> {
        let RelayContext {
            agent_id: agent,
            functions,
            metadata,
            ..
        } = context;
        match self {
            Command::Start { message } => {
                let call = user.chat(agent, message);
                Ok(events(attributed!(call, metadata, functions).await?))
            }
            Command::Resume { session, message } => {
                let call = user.chat(agent, message).session(session);
                Ok(events(attributed!(call, metadata, functions).await?))
            }
            Command::Regenerate {
                session,
                message,
                message_id,
            } => {
                let call = user.regenerate(agent, session, message);
                let call = match message_id {
                    Some(message_id) => call.message_id(message_id),
                    None => call,
                };
                Ok(events(attributed!(call, metadata, functions).await?))
            }
            Command::Approve { session, decisions } => {
                let functions = match functions {
                    Some(functions) => functions,
                    None => Functions::builder()
                        .build()
                        .expect("no functions is always valid"),
                };
                let stream = user
                    .continue_chat(agent, session, decisions)
                    .functions(functions)
                    .await?;
                Ok(events(stream))
            }
            Command::History { session, cursor } => {
                let call = user
                    .sessions()
                    .messages(agent, session)
                    .limit(HISTORY_PAGE_SIZE);
                let page = match cursor {
                    Some(cursor) => call.cursor(Cursor::from(cursor)).await?,
                    None => call.await?,
                };
                Ok(document(
                    StatusCode::OK,
                    &json!({ "data": page.data, "nextCursor": page.next_cursor }),
                ))
            }
            Command::Steer {
                session,
                request_id,
                message,
            } => {
                let receipt = user
                    .sessions()
                    .submit_input(agent, session, request_id, message)
                    .await?;
                Ok(document(
                    StatusCode::ACCEPTED,
                    &json!({ "state": receipt.input.state.as_str() }),
                ))
            }
        }
    }
}

/// The API's status and stream, with the headers the browser reads: `Location`
/// carries a new Session's URL, `X-BA-Turn-Id` the Turn to stop, and
/// `X-Request-Id` the ID to quote to support.
fn events(stream: ChatStream) -> Response<RelayBody> {
    let meta = stream.meta().clone();
    let mut response = Response::new(RelayBody(Repr::Events(Box::new(stream))));
    *response.status_mut() = StatusCode::from_u16(meta.status()).unwrap_or(StatusCode::OK);
    let headers = response.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    headers.insert(
        HeaderName::from_static("x-vercel-ai-ui-message-stream"),
        HeaderValue::from_static("v1"),
    );
    headers.insert(
        HeaderName::from_static("x-accel-buffering"),
        HeaderValue::from_static("no"),
    );
    for name in [LOCATION, X_BA_TURN_ID, X_REQUEST_ID] {
        if let Some(value) = meta.headers().get(&name) {
            headers.insert(name, value.clone());
        }
    }
    response
}

fn document(status: StatusCode, value: &Value) -> Response<RelayBody> {
    let bytes = serde_json::to_vec(value).expect("a JSON value serializes");
    let mut response = Response::new(RelayBody(Repr::Json(Some(Bytes::from(bytes)))));
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    response
}

fn error_response(
    status: StatusCode,
    code: &str,
    message: &str,
    request_id: Option<&str>,
) -> Response<RelayBody> {
    let mut response = document(
        status,
        &json!({ "error": { "code": code, "message": message } }),
    );
    if let Some(request_id) = request_id.and_then(|id| HeaderValue::from_str(id).ok()) {
        response.headers_mut().insert(X_REQUEST_ID, request_id);
    }
    response
}

fn invalid(message: &str) -> Response<RelayBody> {
    error_response(StatusCode::BAD_REQUEST, "invalid_request", message, None)
}

fn unauthorized() -> Response<RelayBody> {
    error_response(
        StatusCode::UNAUTHORIZED,
        "unauthorized",
        "Authentication required.",
        None,
    )
}

/// The browser gets no cause; the log gets it.
fn internal_error(cause: &(impl fmt::Display + ?Sized)) -> Response<RelayBody> {
    tracing::error!(%cause, "the chat relay failed");
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal_error",
        "Request failed.",
        None,
    )
}

/// The API's own refusal keeps its status and code. Failures of the SDK or
/// the network are `502`, the way a gateway reports an upstream it cannot
/// use. Anything else is `500` without its cause.
fn failure(error: Error) -> Response<RelayBody> {
    let request_id = error.request_id();
    match &error {
        Error::Api(api) => error_response(
            StatusCode::from_u16(api.status()).unwrap_or(StatusCode::BAD_GATEWAY),
            api.code().as_str(),
            api.message(),
            request_id,
        ),
        Error::Stream(_) => error_response(
            StatusCode::BAD_GATEWAY,
            "stream_error",
            "The Blazing Agents response stream failed.",
            request_id,
        ),
        Error::Connection(_) | Error::Timeout => error_response(
            StatusCode::BAD_GATEWAY,
            "network_error",
            "Unable to reach Blazing Agents.",
            request_id,
        ),
        Error::Decode { .. } | Error::CredentialEcho { .. } => error_response(
            StatusCode::BAD_GATEWAY,
            "invalid_response",
            "Blazing Agents sent a response this SDK cannot read.",
            request_id,
        ),
        _ => internal_error(&error),
    }
}

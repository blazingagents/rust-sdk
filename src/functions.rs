//! Backend functions: handlers in your process that the agent can call during a
//! chat turn. Build a [`Functions`] value once and attach it to every chat call
//! with [`functions`](crate::call::Call::functions).

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::SystemTime;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize, Serializer};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::call::{Call, ResponseMeta, impl_request};
use crate::error::{Error, Result, StreamError, StreamErrorKind};
use crate::http::Handle;
use crate::ids::{AgentId, FunctionCallId, SessionId};
use crate::op::Op;
use crate::types::Timestamp;

const EVENT_TYPE: &str = "data-ba-function-call";
const MAX_FUNCTIONS: usize = 32;

type HandlerError = Box<dyn std::error::Error + Send + Sync>;
type HandlerFuture =
    Pin<Box<dyn Future<Output = std::result::Result<serde_json::Value, HandlerError>> + Send>>;

trait Handler: Send + Sync {
    fn call(&self, input: serde_json::Value, context: FunctionContext) -> HandlerFuture;
}

impl<F> Handler for F
where
    F: Fn(serde_json::Value, FunctionContext) -> HandlerFuture + Send + Sync,
{
    fn call(&self, input: serde_json::Value, context: FunctionContext) -> HandlerFuture {
        self(input, context)
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Definition {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

/// Validated function definitions and their handlers. Cheap to clone; attach
/// the same value to every chat call.
#[derive(Clone)]
pub struct Functions {
    definitions: Arc<[Definition]>,
    handlers: Arc<BTreeMap<String, Arc<dyn Handler>>>,
}

impl Functions {
    /// Starts defining functions.
    pub fn builder() -> FunctionsBuilder {
        FunctionsBuilder::default()
    }
}

impl Serialize for Functions {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.collect_seq(self.definitions.iter())
    }
}

impl fmt::Debug for Functions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.definitions.iter().map(|definition| &definition.name))
            .finish()
    }
}

/// Collects function definitions and handlers for [`Functions`].
#[derive(Default)]
pub struct FunctionsBuilder {
    definitions: Vec<Definition>,
    handlers: BTreeMap<String, Arc<dyn Handler>>,
}

impl FunctionsBuilder {
    /// Adds a function whose input schema is derived from `I`. The handler's
    /// `Ok` value is sent to the agent as JSON; an `Err` becomes "Function
    /// execution failed." and its text is never sent.
    #[cfg(feature = "schemars")]
    pub fn function<I, F, Fut, O, E>(self, name: &str, description: &str, handler: F) -> Self
    where
        I: DeserializeOwned + schemars::JsonSchema + Send + 'static,
        F: Fn(I, FunctionContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = std::result::Result<O, E>> + Send + 'static,
        O: Serialize,
        E: Into<HandlerError>,
    {
        let schema = serde_json::to_value(schemars::schema_for!(I))
            .expect("a JSON Schema serializes to JSON");
        self.function_json(
            name,
            description,
            schema,
            move |value, context| -> HandlerFuture {
                match serde_json::from_value::<I>(value) {
                    Err(error) => Box::pin(std::future::ready(Err(error.into()))),
                    Ok(input) => {
                        let output = handler(input, context);
                        Box::pin(async move {
                            let output = output.await.map_err(Into::into)?;
                            serde_json::to_value(output).map_err(Into::into)
                        })
                    }
                }
            },
        )
    }

    /// Adds a function with a hand-written JSON Schema and a handler that
    /// receives the input as `serde_json::Value`.
    pub fn function_json<F, Fut, E>(
        mut self,
        name: &str,
        description: &str,
        input_schema: serde_json::Value,
        handler: F,
    ) -> Self
    where
        F: Fn(serde_json::Value, FunctionContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = std::result::Result<serde_json::Value, E>> + Send + 'static,
        E: Into<HandlerError>,
    {
        self.definitions.push(Definition {
            name: name.to_owned(),
            description: description.to_owned(),
            input_schema,
        });
        let erased = move |value, context| -> HandlerFuture {
            let output = handler(value, context);
            Box::pin(async move { output.await.map_err(Into::into) })
        };
        self.handlers.insert(name.to_owned(), Arc::new(erased));
        self
    }

    /// Checks the definitions and produces [`Functions`].
    pub fn build(self) -> std::result::Result<Functions, FunctionsError> {
        if self.definitions.len() > MAX_FUNCTIONS {
            return Err(FunctionsError::TooMany);
        }
        let mut seen = HashSet::new();
        for definition in &self.definitions {
            if !is_function_name(&definition.name) {
                return Err(FunctionsError::InvalidName(definition.name.clone()));
            }
            if definition.name.starts_with("mcp__") {
                return Err(FunctionsError::ReservedName(definition.name.clone()));
            }
            if !seen.insert(definition.name.as_str()) {
                return Err(FunctionsError::Duplicate(definition.name.clone()));
            }
            if !definition.input_schema.is_object() {
                return Err(FunctionsError::NotAnObjectSchema(definition.name.clone()));
            }
        }
        Ok(Functions {
            definitions: self.definitions.into(),
            handlers: Arc::new(self.handlers),
        })
    }
}

/// `^[A-Za-z][A-Za-z0-9_-]{0,63}$`
fn is_function_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && name.len() <= 64
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

/// Why [`FunctionsBuilder::build`] rejected the definitions.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FunctionsError {
    /// A name does not match `^[A-Za-z][A-Za-z0-9_-]{0,63}$`.
    #[error("invalid function name {0:?}")]
    InvalidName(String),
    /// A name is reserved for built-in or MCP tools.
    #[error("function name {0:?} is reserved")]
    ReservedName(String),
    /// Two functions share a name.
    #[error("duplicate function name {0:?}")]
    Duplicate(String),
    /// More than 32 functions.
    #[error("at most 32 functions can be attached")]
    TooMany,
    /// The serialized definitions exceed 64 KiB.
    #[error("function definitions exceed 64 KiB")]
    DefinitionsTooLarge,
    /// An input schema does not describe a JSON object.
    #[error("the input schema of {0:?} does not describe an object")]
    NotAnObjectSchema(String),
}

/// What a handler knows about the call it serves.
#[derive(Clone, Debug)]
pub struct FunctionContext {
    call_id: FunctionCallId,
    deadline: Timestamp,
    cancel: CancellationToken,
}

impl FunctionContext {
    /// The function call ID. Use it to make side effects idempotent across
    /// retries.
    pub fn idempotency_key(&self) -> &FunctionCallId {
        &self.call_id
    }

    /// When the platform stops waiting for the result.
    pub fn deadline(&self) -> Timestamp {
        self.deadline
    }

    /// Resolves when the deadline passes or the chat stream is dropped.
    pub async fn cancelled(&self) {
        self.cancel.cancelled().await;
    }

    /// Whether the deadline passed or the chat stream was dropped.
    pub fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FunctionCall {
    id: FunctionCallId,
    name: String,
    input: serde_json::Value,
    deadline_at: Timestamp,
}

#[derive(Deserialize)]
struct FunctionCallEvent {
    data: FunctionCall,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ClaimFunctionCall {
    claim_request_id: String,
}

#[derive(Deserialize)]
struct ClaimResult {
    claimed: bool,
}
impl_request!(ClaimFunctionCall => ClaimResult; encode = json; decode = json);

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Outcome {
    Output { value: serde_json::Value },
    Error { message: String },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResolveFunctionCall {
    claim_request_id: String,
    outcome: Outcome,
}

#[derive(Deserialize)]
struct ResolveResult {}
impl_request!(ResolveFunctionCall => ResolveResult; encode = json; decode = json);

/// Executes the function calls a chat stream announces. Owned by the stream;
/// dropping it aborts every running call.
pub(crate) struct Runner {
    handle: Handle,
    agent_id: AgentId,
    session_id: SessionId,
    functions: Functions,
    meta: ResponseMeta,
    seen: HashSet<FunctionCallId>,
    tasks: JoinSet<Result<()>>,
    cancel: CancellationToken,
}

impl Runner {
    pub(crate) fn new(
        handle: Handle,
        agent_id: AgentId,
        session_id: SessionId,
        functions: Functions,
        meta: ResponseMeta,
    ) -> Self {
        Self {
            handle,
            agent_id,
            session_id,
            functions,
            meta,
            seen: HashSet::new(),
            tasks: JoinSet::new(),
            cancel: CancellationToken::new(),
        }
    }

    /// Consumes a function-call event and starts its call. Returns `false` for
    /// any other event so the stream yields it.
    pub(crate) fn intercept(&mut self, frame: &[u8]) -> Result<bool> {
        let Some(data) = event_data(frame) else {
            return Ok(false);
        };
        if !data.contains(EVENT_TYPE) {
            return Ok(false);
        }
        let value: serde_json::Value = match serde_json::from_str(&data) {
            Ok(value) => value,
            Err(_) => return Ok(false),
        };
        if value.get("type").and_then(serde_json::Value::as_str) != Some(EVENT_TYPE) {
            return Ok(false);
        }
        let event: FunctionCallEvent = serde_json::from_value(value).map_err(|error| {
            Error::from(StreamError {
                kind: StreamErrorKind::MalformedFunctionCall,
                meta: self.meta.clone(),
                source: Some(Box::new(error)),
            })
        })?;
        if !self.seen.insert(event.data.id.clone()) {
            return Ok(true);
        }
        let handler = self.functions.handlers.get(&event.data.name).cloned();
        self.tasks.spawn(run_call(
            self.handle.clone(),
            self.agent_id.clone(),
            self.session_id.clone(),
            event.data,
            handler,
            self.cancel.child_token(),
        ));
        Ok(true)
    }

    /// The first failed call, when one has finished.
    pub(crate) fn poll_failure(&mut self, cx: &mut Context<'_>) -> Poll<Option<Error>> {
        loop {
            match self.tasks.poll_join_next(cx) {
                Poll::Ready(Some(Ok(Err(error)))) => return Poll::Ready(Some(error)),
                Poll::Ready(Some(_)) => continue,
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

/// The joined `data:` lines of one SSE event.
fn event_data(frame: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(frame).ok()?;
    let lines: Vec<&str> = text
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(|data| data.strip_prefix(' ').unwrap_or(data))
        .collect();
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

async fn run_call(
    handle: Handle,
    agent_id: AgentId,
    session_id: SessionId,
    call: FunctionCall,
    handler: Option<Arc<dyn Handler>>,
    cancel: CancellationToken,
) -> Result<()> {
    let FunctionCall {
        id,
        name,
        input,
        deadline_at,
    } = call;
    let claim_request_id = uuid::Uuid::new_v4().to_string();
    let claimed: ClaimResult = Call::with_handle(
        handle.clone(),
        Op::ClaimChatFunctionCall {
            agent_id: agent_id.as_str(),
            session_id: session_id.as_str(),
            function_call_id: id.as_str(),
        },
        ClaimFunctionCall {
            claim_request_id: claim_request_id.clone(),
        },
    )
    .await?;
    if !claimed.claimed {
        return Ok(());
    }
    let outcome = match handler {
        None => Outcome::Error {
            message: format!("Function {name} is not available."),
        },
        Some(handler) => {
            let context = FunctionContext {
                call_id: id.clone(),
                deadline: deadline_at,
                cancel: cancel.clone(),
            };
            let remaining = deadline_at
                .to_system_time()
                .duration_since(SystemTime::now())
                .unwrap_or_default();
            match tokio::time::timeout(remaining, handler.call(input, context)).await {
                Ok(Ok(value)) => Outcome::Output { value },
                Ok(Err(_)) => Outcome::Error {
                    message: "Function execution failed.".to_owned(),
                },
                Err(_) => {
                    cancel.cancel();
                    return Ok(());
                }
            }
        }
    };
    let _: ResolveResult = Call::with_handle(
        handle,
        Op::ResolveChatFunctionCall {
            agent_id: agent_id.as_str(),
            session_id: session_id.as_str(),
            function_call_id: id.as_str(),
        },
        ResolveFunctionCall {
            claim_request_id,
            outcome,
        },
    )
    .await?;
    Ok(())
}

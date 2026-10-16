//! Backend functions: handlers in your process that the agent can call during a
//! chat turn. Build a [`Functions`] value once and attach it to every chat call
//! with [`functions`](crate::call::Call::functions).
//!
//! Documentation: <https://docs.blazingagents.com/agents/tools/backend-functions>.

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::pin::{Pin, pin};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, SystemTime};

use futures_util::FutureExt;
use futures_util::future::{Either, select};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize, Serializer};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::call::{Call, Raw, Request, ResponseMeta, impl_request};
use crate::error::{Error, Result, StreamError, StreamErrorKind};
use crate::http::Handle;
use crate::ids::{AgentId, FunctionCallId, SessionId};
use crate::op::Op;
use crate::types::Timestamp;

const EVENT_TYPE: &str = "data-ba-function-call";
const MAX_FUNCTIONS: usize = 32;
const MAX_DEFINITIONS_BYTES: usize = 64 * 1024;
const MAX_PAYLOAD_BYTES: usize = 256 * 1024;
/// A result may be submitted this long after the deadline, so a receipt the
/// server already accepted is not lost to one slow retry.
const RESULT_GRACE: Duration = Duration::from_secs(30);
const FIRST_RETRY_DELAY: Duration = Duration::from_millis(250);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(2);
/// Built-in tool names a function must not shadow.
const RESERVED_NAMES: [&str; 15] = [
    "read",
    "write",
    "edit",
    "grep",
    "glob",
    "bash",
    "publish_artifacts",
    "write_todos",
    "save_memory",
    "get_memory",
    "search_memories",
    "update_memory",
    "delete_memory",
    "web_search",
    "activate_skill",
];

type HandlerError = Box<dyn std::error::Error + Send + Sync>;
type HandlerFuture =
    Pin<Box<dyn Future<Output = std::result::Result<serde_json::Value, HandlerFailure>> + Send>>;

/// Why a handler produced no output. Only the sanitized message reaches the
/// agent; the handler's own error text never leaves the process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HandlerFailure {
    InvalidInput,
    Execution,
    InvalidResult,
}

impl HandlerFailure {
    fn message(self) -> &'static str {
        match self {
            HandlerFailure::InvalidInput => "Invalid function input.",
            HandlerFailure::Execution => "Function execution failed.",
            HandlerFailure::InvalidResult => "Function returned an invalid result.",
        }
    }
}

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
    description: String,
    input_schema: serde_json::Value,
}

struct Function {
    definition: Definition,
    handler: Arc<dyn Handler>,
}

/// Validated function definitions and their handlers. Cheap to clone; attach
/// the same value to every chat call.
#[derive(Clone)]
pub struct Functions {
    functions: Arc<BTreeMap<String, Function>>,
}

impl Functions {
    /// Starts defining functions.
    pub fn builder() -> FunctionsBuilder {
        FunctionsBuilder::default()
    }

    fn definitions(&self) -> BTreeMap<&str, &Definition> {
        self.functions
            .iter()
            .map(|(name, function)| (name.as_str(), &function.definition))
            .collect()
    }
}

impl Serialize for Functions {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.collect_map(self.definitions())
    }
}

impl fmt::Debug for Functions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.functions.keys()).finish()
    }
}

/// Collects function definitions and handlers for [`Functions`].
#[derive(Default)]
pub struct FunctionsBuilder {
    functions: Vec<(String, Function)>,
}

impl FunctionsBuilder {
    /// Adds a function whose input schema is derived from `I`. The handler's
    /// `Ok` value is sent to the agent as JSON. Input that does not decode into
    /// `I`, an `Err`, or an output that is not plain JSON becomes a sanitized
    /// tool error; the error's text is never sent.
    #[cfg(feature = "schemars")]
    pub fn function<I, F, Fut, O, E>(self, name: &str, description: &str, handler: F) -> Self
    where
        I: DeserializeOwned + schemars::JsonSchema + Send + 'static,
        F: Fn(I, FunctionContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = std::result::Result<O, E>> + Send + 'static,
        O: Serialize,
        E: Into<HandlerError>,
    {
        let mut schema = serde_json::to_value(schemars::schema_for!(I))
            .expect("a JSON Schema serializes to JSON");
        if let Some(fields) = schema.as_object_mut() {
            fields.remove("$schema");
        }
        self.add(
            name,
            description,
            schema,
            move |value, context| -> HandlerFuture {
                match serde_json::from_value::<I>(value) {
                    Err(_) => Box::pin(std::future::ready(Err(HandlerFailure::InvalidInput))),
                    Ok(input) => {
                        let output = handler(input, context);
                        Box::pin(async move {
                            let output = output.await.map_err(|_| HandlerFailure::Execution)?;
                            plain_json(&output)
                        })
                    }
                }
            },
        )
    }

    /// Adds a function with a hand-written JSON Schema and a handler that
    /// receives the input as `serde_json::Value`. An `Err` becomes a sanitized
    /// tool error; the error's text is never sent.
    pub fn function_json<F, Fut, E>(
        self,
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
        self.add(
            name,
            description,
            input_schema,
            move |value, context| -> HandlerFuture {
                let output = handler(value, context);
                Box::pin(async move { output.await.map_err(|_| HandlerFailure::Execution) })
            },
        )
    }

    fn add(
        mut self,
        name: &str,
        description: &str,
        input_schema: serde_json::Value,
        handler: impl Handler + 'static,
    ) -> Self {
        self.functions.push((
            name.to_owned(),
            Function {
                definition: Definition {
                    description: description.to_owned(),
                    input_schema,
                },
                handler: Arc::new(handler),
            },
        ));
        self
    }

    /// Checks the definitions against the platform's limits and produces
    /// [`Functions`].
    pub fn build(self) -> std::result::Result<Functions, FunctionsError> {
        if self.functions.len() > MAX_FUNCTIONS {
            return Err(FunctionsError::TooMany);
        }
        let mut functions = BTreeMap::new();
        for (name, function) in self.functions {
            if !is_function_name(&name) {
                return Err(FunctionsError::InvalidName(name));
            }
            if is_reserved_name(&name) {
                return Err(FunctionsError::ReservedName(name));
            }
            if function.definition.description.trim().is_empty() {
                return Err(FunctionsError::EmptyDescription(name));
            }
            if function.definition.input_schema.get("type") != Some(&"object".into()) {
                return Err(FunctionsError::NotAnObjectSchema(name));
            }
            if functions.contains_key(&name) {
                return Err(FunctionsError::Duplicate(name));
            }
            functions.insert(name, function);
        }
        let functions = Functions {
            functions: Arc::new(functions),
        };
        let encoded = serde_json::to_vec(&functions.definitions())
            .expect("function definitions serialize to JSON");
        if encoded.len() > MAX_DEFINITIONS_BYTES {
            return Err(FunctionsError::DefinitionsTooLarge);
        }
        Ok(functions)
    }
}

/// `output` as JSON text parsed back into a value, so only what can be sent
/// on the wire is accepted. Non-finite floats still become `null`: serde_json
/// writes them that way and offers no strict mode.
fn plain_json(output: &impl Serialize) -> std::result::Result<serde_json::Value, HandlerFailure> {
    let bytes = serde_json::to_vec(output).map_err(|_| HandlerFailure::InvalidResult)?;
    serde_json::from_slice(&bytes).map_err(|_| HandlerFailure::InvalidResult)
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

fn is_reserved_name(name: &str) -> bool {
    RESERVED_NAMES.contains(&name) || name.starts_with("mcp__")
}

/// Whether `value` is `prefix` followed by 16 alphanumeric characters.
pub(crate) fn is_platform_id(value: &str, prefix: &str) -> bool {
    value.strip_prefix(prefix).is_some_and(|rest| {
        rest.len() == 16 && rest.bytes().all(|byte| byte.is_ascii_alphanumeric())
    })
}

/// Why [`FunctionsBuilder::build`] rejected the definitions.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FunctionsError {
    /// A name does not match `^[A-Za-z][A-Za-z0-9_-]{0,63}$`.
    #[error("invalid function name {0:?}")]
    InvalidName(String),
    /// A name is a built-in tool's name or starts with `mcp__`.
    #[error("function name {0:?} is reserved for a built-in or MCP tool")]
    ReservedName(String),
    /// Two functions share a name.
    #[error("duplicate function name {0:?}")]
    Duplicate(String),
    /// More than 32 functions.
    #[error("at most 32 functions can be attached")]
    TooMany,
    /// A description is empty or only whitespace.
    #[error("the description of {0:?} is empty")]
    EmptyDescription(String),
    /// The serialized definitions exceed 64 KiB.
    #[error("function definitions exceed 64 KiB")]
    DefinitionsTooLarge,
    /// An input schema's `type` is not `object`.
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

impl FunctionCall {
    fn is_well_formed(&self) -> bool {
        is_platform_id(self.id.as_str(), "fc_")
            && is_function_name(&self.name)
            && !is_reserved_name(&self.name)
    }
}

#[derive(Deserialize)]
struct FunctionCallEvent {
    data: FunctionCall,
    transient: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClaimFunctionCall {
    claim_request_id: String,
}

impl_request!(ClaimFunctionCall => (); encode = json; decode = (|raw| decode_true(raw, "claimed")));

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Outcome {
    Output { value: serde_json::Value },
    Error { message: String },
}

impl Outcome {
    fn failure(failure: HandlerFailure) -> Self {
        Outcome::Error {
            message: failure.message().to_owned(),
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResolveFunctionCall {
    claim_request_id: String,
    outcome: Outcome,
}

impl_request!(ResolveFunctionCall => (); encode = json; decode = (|raw| decode_true(raw, "accepted")));

/// Decodes a `{ "<field>": true }` receipt. Any other body is a decode
/// failure, which is permanent.
async fn decode_true(raw: Raw, field: &'static str) -> Result<()> {
    let operation_id = raw.operation_id;
    let request_id = raw.meta.request_id.clone();
    let value: serde_json::Value = raw.json().await?;
    if value.get(field) == Some(&true.into()) {
        return Ok(());
    }
    Err(Error::Decode {
        operation_id,
        request_id,
        source: serde::de::Error::custom(format!("expected {field}: true")),
    })
}

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
        let malformed = || {
            Error::from(StreamError {
                kind: StreamErrorKind::MalformedFunctionCall,
                meta: self.meta.clone(),
                source: None,
            })
        };
        let value: serde_json::Value = serde_json::from_str(&data).map_err(|_| malformed())?;
        if value.get("type").and_then(serde_json::Value::as_str) != Some(EVENT_TYPE) {
            return Ok(false);
        }
        let event = serde_json::from_value::<FunctionCallEvent>(value)
            .ok()
            .filter(|event| event.transient && event.data.is_well_formed())
            .ok_or_else(malformed)?;
        if !self.seen.insert(event.data.id.clone()) {
            return Ok(true);
        }
        let handler = self
            .functions
            .functions
            .get(&event.data.name)
            .map(|function| function.handler.clone());
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
    let text = String::from_utf8_lossy(frame);
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

/// How a claim or result submission ended. Only `Failed` reaches the stream's
/// consumer.
enum Post {
    Accepted,
    /// The server answered 409: another process holds the call, or it is stale.
    Refused,
    /// The budget ran out or the stream was dropped before an answer.
    Expired,
    /// A permanent failure, such as a 4xx other than 408, 409 and 429.
    Failed(Error),
}

/// The delay before retrying a failed submission, or `None` when the failure
/// is permanent. `Retry-After` wins over the exponential backoff.
fn retry_delay(error: &Error, attempt: u32) -> Option<Duration> {
    match error {
        Error::Api(api) => {
            let status = api.status();
            if status < 500 && status != 408 && status != 429 {
                return None;
            }
            if let Some(retry_after) = api.retry_after() {
                return Some(retry_after.max(FIRST_RETRY_DELAY));
            }
        }
        Error::Connection(_) | Error::Timeout | Error::Stream(_) => {}
        _ => return None,
    }
    Some((FIRST_RETRY_DELAY * 2u32.saturating_pow(attempt)).min(MAX_RETRY_DELAY))
}

/// Sends `call` until it is accepted, retrying transient failures with the
/// same body until `until` passes or `cancel` fires.
async fn post<R: Request + Clone>(
    action: &str,
    call_id: &FunctionCallId,
    call: Call<R>,
    until: SystemTime,
    cancel: &CancellationToken,
) -> Post {
    let mut attempt = 0;
    loop {
        if cancel.is_cancelled() || SystemTime::now() >= until {
            return Post::Expired;
        }
        let error = match call.clone().await {
            Ok(_) => return Post::Accepted,
            Err(error) => error,
        };
        if matches!(&error, Error::Api(api) if api.status() == 409) {
            tracing::debug!(call_id = %call_id, action, "function call refused");
            return Post::Refused;
        }
        let Some(delay) = retry_delay(&error, attempt) else {
            tracing::warn!(call_id = %call_id, action, error = %error, "function call failed");
            return Post::Failed(error);
        };
        if SystemTime::now() + delay >= until {
            tracing::warn!(call_id = %call_id, action, error = %error, "function call expired");
            return Post::Expired;
        }
        if cancel
            .run_until_cancelled(tokio::time::sleep(delay))
            .await
            .is_none()
        {
            return Post::Expired;
        }
        attempt += 1;
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
    let deadline = deadline_at.to_system_time();
    let claim_request_id = uuid::Uuid::new_v4().to_string();
    let claim = Call::with_handle(
        handle.clone(),
        Op::ClaimChatFunctionCall {
            agent_id: agent_id.as_str(),
            session_id: session_id.as_str(),
            function_call_id: id.as_str(),
        },
        ClaimFunctionCall {
            claim_request_id: claim_request_id.clone(),
        },
    );
    match post("claim", &id, claim, deadline, &cancel).await {
        Post::Accepted => {}
        Post::Refused | Post::Expired => return Ok(()),
        Post::Failed(error) => return Err(error),
    }
    if cancel.is_cancelled() || SystemTime::now() >= deadline {
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
            let remaining = deadline
                .duration_since(SystemTime::now())
                .unwrap_or_default();
            let run = AssertUnwindSafe(handler.call(input, context)).catch_unwind();
            let finished = match select(pin!(run), pin!(tokio::time::sleep(remaining))).await {
                Either::Left((finished, _)) => finished,
                Either::Right(_) => {
                    // The handler is dropped when `run` goes out of scope, after the cancel.
                    cancel.cancel();
                    return Ok(());
                }
            };
            match finished.unwrap_or(Err(HandlerFailure::Execution)) {
                Ok(value) => output_outcome(&name, &id, value),
                Err(failure) => {
                    tracing::warn!(function = name, call_id = %id, "function {}", failure.message());
                    Outcome::failure(failure)
                }
            }
        }
    };
    let result = Call::with_handle(
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
    );
    match post("result", &id, result, deadline + RESULT_GRACE, &cancel).await {
        Post::Failed(error) => Err(error),
        _ => Ok(()),
    }
}

/// The output outcome, unless it exceeds the payload limit.
fn output_outcome(name: &str, id: &FunctionCallId, value: serde_json::Value) -> Outcome {
    let outcome = Outcome::Output { value };
    let size = serde_json::to_vec(&outcome)
        .expect("a JSON value serializes")
        .len();
    if size > MAX_PAYLOAD_BYTES {
        tracing::warn!(function = name, call_id = %id, size, "function result exceeds {MAX_PAYLOAD_BYTES} bytes");
        return Outcome::failure(HandlerFailure::InvalidResult);
    }
    outcome
}

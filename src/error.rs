//! Errors returned by the SDK.

use std::fmt;
use std::time::Duration;

use http::HeaderMap;

use crate::call::ResponseMeta;

mod code;

pub use code::ErrorCode;

/// `Result` with the SDK's [`Error`] as the default error type.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Every failure a call can produce.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A non-2xx response with the API error envelope.
    #[error(transparent)]
    Api(Box<ApiError>),
    /// A network failure before a complete response.
    #[error("connection failed")]
    Connection(#[source] Box<dyn std::error::Error + Send + Sync>),
    /// The call's timeout elapsed.
    #[error("request timed out")]
    Timeout,
    /// A failure after the response headers arrived: a body read error, a missing or
    /// malformed stream header, a malformed function-call event, or an incomplete body.
    #[error(transparent)]
    Stream(Box<StreamError>),
    /// Object generation produced truncated JSON, invalid JSON, or JSON that does not
    /// match the requested type.
    #[error(transparent)]
    Object(Box<ObjectError>),
    /// A 2xx body that does not match the response model.
    #[error("unexpected response body for {operation_id}")]
    Decode {
        /// The spec's `operationId`.
        operation_id: &'static str,
        /// The server request ID from `X-Request-Id`.
        request_id: Option<String>,
        /// The decoding failure.
        #[source]
        source: serde_json::Error,
    },
    /// A response that echoed credential material back. The body is discarded.
    #[error("the server returned credential material in a response to {operation_id}")]
    CredentialEcho {
        /// The spec's `operationId`.
        operation_id: &'static str,
        /// The server request ID from `X-Request-Id`.
        request_id: Option<String>,
    },
    /// A client-side argument the types could not rule out, such as a path
    /// parameter of `..`.
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    /// A client configuration problem.
    #[error(transparent)]
    Config(#[from] ConfigError),
}

impl Error {
    /// The HTTP status, when a response was received.
    pub fn status(&self) -> Option<u16> {
        match self {
            Error::Api(error) => Some(error.status),
            Error::Stream(error) => Some(error.meta.status()),
            Error::Object(error) => Some(error.meta.status()),
            _ => None,
        }
    }

    /// The server request ID, when a response was received.
    pub fn request_id(&self) -> Option<&str> {
        match self {
            Error::Api(error) => error.request_id(),
            Error::Stream(error) => error.meta.request_id(),
            Error::Object(error) => error.meta.request_id(),
            Error::Decode { request_id, .. } | Error::CredentialEcho { request_id, .. } => {
                request_id.as_deref()
            }
            _ => None,
        }
    }

    /// The API error code, for [`Error::Api`].
    pub fn code(&self) -> Option<&ErrorCode> {
        match self {
            Error::Api(error) => Some(&error.code),
            _ => None,
        }
    }
}

impl From<ApiError> for Error {
    fn from(error: ApiError) -> Self {
        Error::Api(Box::new(error))
    }
}

impl From<StreamError> for Error {
    fn from(error: StreamError) -> Self {
        Error::Stream(Box::new(error))
    }
}

impl From<ObjectError> for Error {
    fn from(error: ObjectError) -> Self {
        Error::Object(Box::new(error))
    }
}

/// A non-2xx response. Submitted credentials are redacted from [`body`](Self::body)
/// and [`headers`](Self::headers).
#[derive(Debug)]
pub struct ApiError {
    pub(crate) status: u16,
    pub(crate) code: ErrorCode,
    pub(crate) message: String,
    pub(crate) details: Option<serde_json::Map<String, serde_json::Value>>,
    pub(crate) param: Option<String>,
    pub(crate) request_id: Option<String>,
    pub(crate) retry_after: Option<Duration>,
    pub(crate) headers: HeaderMap,
    pub(crate) body: String,
}

impl ApiError {
    /// The HTTP status.
    pub fn status(&self) -> u16 {
        self.status
    }

    /// The `error.code` of the envelope. Branch on this, not on the message.
    pub fn code(&self) -> &ErrorCode {
        &self.code
    }

    /// The `error.message` of the envelope.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The `error.details` object, when present.
    pub fn details(&self) -> Option<&serde_json::Map<String, serde_json::Value>> {
        self.details.as_ref()
    }

    /// The request parameter the error names, when present.
    pub fn param(&self) -> Option<&str> {
        self.param.as_deref()
    }

    /// The server request ID from `X-Request-Id`.
    pub fn request_id(&self) -> Option<&str> {
        self.request_id.as_deref()
    }

    /// `Retry-After`, parsed as delta-seconds or an HTTP-date.
    pub fn retry_after(&self) -> Option<Duration> {
        self.retry_after
    }

    /// The response headers.
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// The response body.
    pub fn body(&self) -> &str {
        &self.body
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for ApiError {}

/// A failure after stream headers arrived.
#[derive(Debug)]
pub struct StreamError {
    pub(crate) kind: StreamErrorKind,
    pub(crate) meta: ResponseMeta,
    pub(crate) source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl StreamError {
    /// What failed.
    pub fn kind(&self) -> StreamErrorKind {
        self.kind
    }

    /// Status, headers and request ID of the response.
    pub fn meta(&self) -> &ResponseMeta {
        &self.meta
    }
}

impl fmt::Display for StreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.kind {
            StreamErrorKind::Read => "response body read failed",
            StreamErrorKind::Incomplete => "the stream ended before it finished",
            StreamErrorKind::MissingSessionId => "the response has no Location header",
            StreamErrorKind::MalformedSessionId => "the Location header is not a session URL",
            StreamErrorKind::MissingTurnId => "the response has no X-BA-Turn-Id header",
            StreamErrorKind::MalformedTurnId => "the X-BA-Turn-Id header is not a turn ID",
            StreamErrorKind::MalformedFunctionCall => "a function-call event is malformed",
        })
    }
}

impl std::error::Error for StreamError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_deref().map(|error| error as _)
    }
}

/// The ways a stream fails after its headers arrived.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum StreamErrorKind {
    /// Reading the body failed.
    Read,
    /// The body ended before the stream finished cleanly.
    Incomplete,
    /// A new chat session's response has no `Location` header.
    MissingSessionId,
    /// The `Location` header does not end in a session ID.
    MalformedSessionId,
    /// A chat response has no `X-BA-Turn-Id` header.
    MissingTurnId,
    /// The `X-BA-Turn-Id` header is not a turn ID.
    MalformedTurnId,
    /// A `data-ba-function-call` event does not match the expected shape.
    MalformedFunctionCall,
}

/// Object generation finished, but the text is not the requested object.
#[derive(Debug)]
pub struct ObjectError {
    pub(crate) kind: ObjectErrorKind,
    pub(crate) text: String,
    pub(crate) meta: ResponseMeta,
    pub(crate) source: serde_json::Error,
}

impl ObjectError {
    /// What failed.
    pub fn kind(&self) -> ObjectErrorKind {
        self.kind
    }

    /// The complete generated text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Status, headers and request ID of the response.
    pub fn meta(&self) -> &ResponseMeta {
        &self.meta
    }
}

impl fmt::Display for ObjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.kind {
            ObjectErrorKind::Truncated => "the agent produced truncated JSON",
            ObjectErrorKind::InvalidJson => "the agent produced invalid JSON",
            ObjectErrorKind::Mismatch => {
                "the generated object did not match the requested output type"
            }
        })
    }
}

impl std::error::Error for ObjectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// The ways generated JSON fails to become the requested object.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ObjectErrorKind {
    /// The text ends in the middle of a JSON value.
    Truncated,
    /// The text is not JSON.
    InvalidJson,
    /// The JSON parsed but does not match the requested type.
    Mismatch,
}

/// A client configuration problem, reported before any request is sent.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ConfigError {
    /// No `api_key` was given and `BLAZING_AGENTS_API_KEY` is unset or empty.
    #[error("no API key: pass api_key or set BLAZING_AGENTS_API_KEY")]
    MissingApiKey,
    /// The base URL is not an absolute `http` or `https` URL.
    #[error("invalid base URL: {0}")]
    InvalidBaseUrl(String),
    /// A default header name or value is not valid HTTP.
    #[error("invalid default header: {0}")]
    InvalidHeader(String),
    /// A user ID is empty, longer than 256 bytes, not printable ASCII, or has a
    /// leading or trailing space.
    #[error("invalid user ID: 1 to 256 printable ASCII characters without surrounding spaces")]
    InvalidUserId,
    /// The HTTP client could not be built.
    #[error("cannot build the HTTP client: {0}")]
    HttpClient(String),
}

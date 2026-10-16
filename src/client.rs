//! The client, its builder, and the tenant and user scopes.

use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

use http::{HeaderMap, HeaderName, HeaderValue};

use crate::error::ConfigError;
use crate::http::{Handle, Inner, Observer};
use crate::ids::{AgentId, UserId};

/// The default API base URL.
pub const DEFAULT_BASE_URL: &str = "https://api.blazingagents.com";

/// The environment variable [`Client::from_env`] and the builder read the API
/// key from.
pub const API_KEY_ENV: &str = "BLAZING_AGENTS_API_KEY";

/// The default timeout for calls whose response is not a stream.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// A client for the Blazing Agents API. `Client<Tenant>` (the default) has every
/// operation; [`for_user`](Self::for_user) narrows it to a [`UserClient`] that
/// acts for one end user. Cloning is cheap and shares the connection pool.
pub struct Client<S: Scope = Tenant> {
    handle: Handle,
    _scope: PhantomData<fn() -> S>,
}

/// A client that acts for one end user. It sends `X-BA-User-Id` on every request
/// and exposes no tenant administration.
pub type UserClient = Client<User>;

/// Marker for what a [`Client`] may do. Implemented by [`Tenant`] and [`User`].
pub trait Scope: sealed::Sealed + Send + Sync + 'static {}

/// The whole tenant. See [`Client`].
#[derive(Clone, Copy, Debug)]
pub struct Tenant;

/// One end user. See [`UserClient`].
#[derive(Clone, Copy, Debug)]
pub struct User;

impl Scope for Tenant {}
impl Scope for User {}

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::Tenant {}
    impl Sealed for super::User {}
}

impl Client<Tenant> {
    /// A client configured from `BLAZING_AGENTS_API_KEY` with every default.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::builder().build()
    }

    /// Starts configuring a client.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// A client that acts for `user_id`: 1 to 256 printable ASCII characters
    /// without a leading or trailing space. Pass the ID your backend verified,
    /// never one taken from a request body. The scope header overrides any
    /// default or per-call header.
    pub fn for_user(&self, user_id: impl AsRef<str>) -> Result<UserClient, ConfigError> {
        let user_id = UserId::new(user_id.as_ref())?;
        Ok(Client {
            handle: Handle {
                user_id: Some(user_id),
                ..self.handle.clone()
            },
            _scope: PhantomData,
        })
    }
}

impl<S: Scope> Client<S> {
    /// The same client, sending `X-Client-Request-Id` on every call.
    pub fn with_client_request_id(&self, id: impl Into<String>) -> Self {
        Self {
            handle: Handle {
                client_request_id: Some(id.into()),
                ..self.handle.clone()
            },
            _scope: PhantomData,
        }
    }

    /// Resources scoped to one agent, such as its skills.
    pub fn agent(&self, agent_id: impl Into<AgentId>) -> AgentHandle<'_, S> {
        AgentHandle {
            client: self,
            agent_id: agent_id.into(),
        }
    }

    pub(crate) fn handle(&self) -> &Handle {
        &self.handle
    }
}

impl<S: Scope> Clone for Client<S> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            _scope: PhantomData,
        }
    }
}

impl<S: Scope> fmt::Debug for Client<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("base_url", &self.handle.inner.base_url)
            .field("user_id", &self.handle.user_id)
            .finish_non_exhaustive()
    }
}

/// Resources of one agent. Returned by [`Client::agent`].
#[derive(Clone, Debug)]
pub struct AgentHandle<'a, S: Scope> {
    pub(crate) client: &'a Client<S>,
    pub(crate) agent_id: AgentId,
}

/// What the client reports to the `on_response` callback: one observation per
/// response, including API errors and stream handshakes. A connection failure
/// produces no observation.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ResponseObservation {
    /// The spec's `operationId`.
    pub operation_id: &'static str,
    /// The HTTP method.
    pub method: &'static str,
    /// The request path without the query.
    pub path: String,
    /// The HTTP status.
    pub status: u16,
    /// Time from sending the request to receiving the headers.
    pub duration: Duration,
    /// The server request ID from `X-Request-Id`.
    pub request_id: Option<String>,
    /// The `X-Client-Request-Id` the call sent.
    pub client_request_id: Option<String>,
}

/// Configures a [`Client`]. Building makes no network request.
pub struct ClientBuilder {
    api_key: Option<String>,
    base_url: String,
    timeout: Option<Duration>,
    connect_timeout: Option<Duration>,
    default_headers: Result<HeaderMap, ConfigError>,
    on_response: Option<Box<Observer>>,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self {
            api_key: None,
            base_url: DEFAULT_BASE_URL.to_owned(),
            timeout: Some(DEFAULT_TIMEOUT),
            connect_timeout: None,
            default_headers: Ok(HeaderMap::new()),
            on_response: None,
        }
    }
}

impl ClientBuilder {
    /// The tenant API key. Wins over `BLAZING_AGENTS_API_KEY`.
    #[must_use]
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// The API base URL. Defaults to [`DEFAULT_BASE_URL`].
    #[must_use]
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// The timeout for calls whose response is not a stream, covering the request
    /// and the body read. Defaults to 60 seconds; `None` disables it. A stream's
    /// timeout covers only the wait for headers, and reading it has no deadline.
    #[must_use]
    pub fn timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        self
    }

    /// The TCP connect timeout. Unset by default.
    #[must_use]
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = Some(timeout);
        self
    }

    /// A header sent on every request. `Authorization`, `X-Request-Id` and
    /// `X-BA-User-Id` are owned by the SDK and ignored here. An invalid name or
    /// value fails [`build`](Self::build).
    #[must_use]
    pub fn default_header(mut self, name: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        if self.default_headers.is_err() {
            return self;
        }
        let parsed = HeaderName::from_bytes(name.as_ref().as_bytes())
            .ok()
            .zip(HeaderValue::from_str(value.as_ref()).ok());
        match parsed {
            Some((name, value)) => {
                if let Ok(headers) = &mut self.default_headers {
                    headers.append(name, value);
                }
            }
            None => {
                self.default_headers = Err(ConfigError::InvalidHeader(name.as_ref().to_owned()))
            }
        }
        self
    }

    /// A callback that sees every response. A panic inside it is caught and
    /// discarded.
    #[must_use]
    pub fn on_response(
        mut self,
        callback: impl Fn(&ResponseObservation) + Send + Sync + 'static,
    ) -> Self {
        self.on_response = Some(Box::new(callback));
        self
    }

    /// Builds the client.
    pub fn build(self) -> Result<Client, ConfigError> {
        let api_key = self
            .api_key
            .or_else(|| std::env::var(API_KEY_ENV).ok())
            .filter(|key| !key.is_empty())
            .ok_or(ConfigError::MissingApiKey)?;
        reqwest::header::HeaderValue::from_str(&format!("Bearer {api_key}"))
            .map_err(|_| ConfigError::InvalidApiKey)?;
        let base_url = self.base_url.trim_end_matches('/').to_owned();
        let parsed = reqwest::Url::parse(&base_url)
            .map_err(|_| ConfigError::InvalidBaseUrl(self.base_url.clone()))?;
        if !matches!(parsed.scheme(), "http" | "https") || parsed.host().is_none() {
            return Err(ConfigError::InvalidBaseUrl(self.base_url));
        }
        let mut http = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none());
        if let Some(timeout) = self.connect_timeout {
            http = http.connect_timeout(timeout);
        }
        let http = http
            .build()
            .map_err(|error| ConfigError::HttpClient(error.without_url().to_string()))?;
        Ok(Client {
            handle: Handle {
                inner: Arc::new(Inner {
                    http,
                    base_url,
                    api_key,
                    default_headers: self.default_headers?,
                    timeout: self.timeout,
                    on_response: self.on_response,
                }),
                client_request_id: None,
                user_id: None,
            },
            _scope: PhantomData,
        })
    }
}

impl fmt::Debug for ClientBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientBuilder")
            .field("base_url", &self.base_url)
            .field("timeout", &self.timeout)
            .field("connect_timeout", &self.connect_timeout)
            .finish_non_exhaustive()
    }
}

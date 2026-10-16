//! The transport: header policy, timeouts, response observation, error envelopes,
//! credential redaction and body decoding.

use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use bytes::Bytes;
use http::header::{AUTHORIZATION, CONTENT_TYPE, HeaderName, USER_AGENT};
use http::{HeaderMap, HeaderValue, Method};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, PercentEncode, utf8_percent_encode};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::call::{Body, Call, InvalidSegment, Raw, Request, ResponseMeta, Route, WithMeta};
use crate::client::ResponseObservation;
use crate::error::{ApiError, Error, ErrorCode, Result, StreamError, StreamErrorKind};
use crate::files::Download;
use crate::ids::UserId;

const X_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");
const X_CLIENT_REQUEST_ID: HeaderName = HeaderName::from_static("x-client-request-id");
const X_BA_USER_ID: HeaderName = HeaderName::from_static("x-ba-user-id");
const RETRY_AFTER: HeaderName = HeaderName::from_static("retry-after");

/// A secret shorter than this is redacted only on an exact match, because a
/// short substring would redact unrelated text.
const MIN_SUBSTRING_SECRET_LENGTH: usize = 8;

/// Response field names that must never carry a value back to the caller.
const CREDENTIAL_FIELDS: [&str; 12] = [
    "apiKey",
    "api_key",
    "botToken",
    "bot_token",
    "signingSecret",
    "signing_secret",
    "webhookSecret",
    "webhook_secret",
    "bearerToken",
    "bearer_token",
    "clientSecret",
    "client_secret",
];

/// Everything but RFC 3986 unreserved characters is encoded, so a value is always
/// exactly one path segment.
const SEGMENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

pub(crate) type Observer = dyn Fn(&ResponseObservation) + Send + Sync;

pub(crate) struct Inner {
    pub(crate) http: reqwest::Client,
    pub(crate) base_url: String,
    pub(crate) api_key: String,
    pub(crate) default_headers: HeaderMap,
    pub(crate) timeout: Option<Duration>,
    pub(crate) on_response: Option<Box<Observer>>,
}

/// A client view: the shared transport plus the per-view headers.
#[derive(Clone)]
pub(crate) struct Handle {
    pub(crate) inner: Arc<Inner>,
    pub(crate) client_request_id: Option<String>,
    pub(crate) user_id: Option<UserId>,
}

/// Per-call transport options. `timeout: Some(None)` disables the timeout.
#[derive(Clone, Default)]
pub(crate) struct RequestOptions {
    pub(crate) timeout: Option<Option<Duration>>,
    pub(crate) headers: HeaderMap,
    pub(crate) client_request_id: Option<String>,
}

/// Encodes one path parameter, or returns its name when URL normalization would
/// change the path.
pub(crate) fn segment<'a>(
    value: &'a str,
    name: &'static str,
) -> std::result::Result<PercentEncode<'a>, &'static str> {
    if value.is_empty() || value == "." || value == ".." {
        return Err(name);
    }
    Ok(utf8_percent_encode(value, SEGMENT))
}

pub(crate) async fn send<R: Request>(call: Call<R>) -> Result<WithMeta<R::Output>> {
    let Call {
        handle,
        route,
        request,
        options,
    } = call;
    let raw_options = options.clone();
    let route = route.map_err(|InvalidSegment(name)| {
        Error::InvalidArgument(format!(
            "path parameter {name} must not be empty, `.` or `..`"
        ))
    })?;
    let inner = &handle.inner;
    let sensitive = request.sensitive();
    let (payload, state) = request.encode()?;

    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
    headers.extend(inner.default_headers.clone());
    headers.extend(options.headers);
    headers.extend(payload.headers);
    headers.remove(X_REQUEST_ID);
    headers.remove(X_BA_USER_ID);
    headers.remove(CONTENT_TYPE);
    let mut authorization =
        HeaderValue::from_str(&format!("Bearer {}", inner.api_key)).map_err(|_| {
            Error::Config(crate::error::ConfigError::HttpClient(
                "the API key is not a valid header value".to_owned(),
            ))
        })?;
    authorization.set_sensitive(true);
    headers.insert(AUTHORIZATION, authorization);
    if let Some(id) = options
        .client_request_id
        .or_else(|| handle.client_request_id.clone())
    {
        headers.insert(
            X_CLIENT_REQUEST_ID,
            HeaderValue::from_str(&id).map_err(|_| {
                Error::InvalidArgument("client request ID is not a valid header value".to_owned())
            })?,
        );
    }
    if let Some(user_id) = &handle.user_id {
        headers.insert(
            X_BA_USER_ID,
            HeaderValue::from_str(user_id.as_str())
                .expect("a validated user ID is printable ASCII"),
        );
    }

    let client_request_id = header_string(&headers, &X_CLIENT_REQUEST_ID);

    let mut url = reqwest::Url::parse(&format!("{}{}", inner.base_url, route.path))
        .map_err(|error| Error::InvalidArgument(format!("invalid request URL: {error}")))?;
    if !payload.query.is_empty() {
        url.query_pairs_mut().extend_pairs(payload.query);
    }
    let method = Method::from_bytes(route.method.as_bytes())
        .expect("the generated operation table only holds standard methods");
    let mut builder = inner.http.request(method, url).headers(headers);
    builder = match payload.body {
        Body::Empty => builder,
        Body::Json(bytes) => builder
            .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
            .body(bytes),
        Body::Multipart(form) => builder.multipart(form),
        Body::OctetStream(body) => builder
            .header(
                CONTENT_TYPE,
                HeaderValue::from_static("application/octet-stream"),
            )
            .body(body),
    };

    let timeout = options.timeout.unwrap_or(inner.timeout);
    let started = Instant::now();
    let deadline = timeout.map(|timeout| tokio::time::Instant::now() + timeout);
    let response = with_deadline(deadline, builder.send())
        .await?
        .map_err(connection_error)?;
    let meta = ResponseMeta {
        status: response.status().as_u16(),
        headers: response.headers().clone(),
        request_id: header_string(response.headers(), &X_REQUEST_ID),
    };
    observe(inner, &route, &meta, started.elapsed(), client_request_id);

    if !response.status().is_success() {
        let body = with_deadline(deadline, response.bytes()).await?;
        return Err(match body {
            Ok(body) => api_error(meta, &body, &sensitive).into(),
            Err(error) => read_error(meta, error),
        });
    }
    let raw = Raw {
        response,
        meta: meta.clone(),
        sensitive,
        operation_id: route.op_id,
        handle: handle.clone(),
        options: raw_options,
    };
    let value = if R::STREAMING {
        R::decode(state, raw).await?
    } else {
        with_deadline(deadline, R::decode(state, raw)).await??
    };
    Ok(WithMeta { value, meta })
}

const USER_AGENT_VALUE: &str = concat!("blazing-agents-rust/", env!("CARGO_PKG_VERSION"));

async fn with_deadline<T>(
    deadline: Option<tokio::time::Instant>,
    future: impl Future<Output = T>,
) -> Result<T> {
    match deadline {
        Some(deadline) => tokio::time::timeout_at(deadline, future)
            .await
            .map_err(|_| Error::Timeout),
        None => Ok(future.await),
    }
}

fn connection_error(error: reqwest::Error) -> Error {
    if error.is_timeout() {
        Error::Timeout
    } else {
        Error::Connection(Box::new(error.without_url()))
    }
}

fn read_error(meta: ResponseMeta, error: reqwest::Error) -> Error {
    StreamError {
        kind: StreamErrorKind::Read,
        meta,
        source: Some(Box::new(error.without_url())),
    }
    .into()
}

fn observe(
    inner: &Inner,
    route: &Route,
    meta: &ResponseMeta,
    duration: Duration,
    client_request_id: Option<String>,
) {
    tracing::debug!(
        operation = route.op_id,
        method = route.method,
        path = %route.path,
        status = meta.status,
        elapsed_ms = duration.as_secs_f64() * 1_000.0,
        request_id = ?meta.request_id,
        "HTTP response"
    );
    let Some(on_response) = &inner.on_response else {
        return;
    };
    let observation = ResponseObservation {
        operation_id: route.op_id,
        method: route.method,
        path: route.path.clone(),
        status: meta.status,
        duration,
        request_id: meta.request_id.clone(),
        client_request_id,
    };
    let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| on_response(&observation)));
    if outcome.is_err() {
        tracing::warn!("the on_response callback panicked; the panic was discarded");
    }
}

fn header_string(headers: &HeaderMap, name: &HeaderName) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

fn api_error(meta: ResponseMeta, body: &[u8], sensitive: &[String]) -> ApiError {
    let raw = String::from_utf8_lossy(body);
    let mut code = ErrorCode::from("invalid_response");
    let mut message = "The server returned an invalid error response.".to_owned();
    let mut details = None;
    let mut param = None;
    let body = match serde_json::from_str::<Value>(&raw) {
        Ok(parsed) => {
            let parsed = redact_value(parsed, sensitive);
            let envelope = parsed.get("error").and_then(Value::as_object);
            let code_and_message = envelope.and_then(|error| {
                Some((
                    error.get("code")?.as_str()?,
                    error.get("message")?.as_str()?,
                ))
            });
            if let (Some(error), Some((parsed_code, parsed_message))) = (envelope, code_and_message)
            {
                code = ErrorCode::from(parsed_code);
                message = parsed_message.to_owned();
                details = error.get("details").and_then(Value::as_object).cloned();
                param = error
                    .get("param")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
            }
            parsed.to_string()
        }
        Err(_) if sensitive.is_empty() => raw.into_owned(),
        Err(_) => "[REDACTED]".to_owned(),
    };
    let headers = redact_headers(meta.headers, sensitive);
    ApiError {
        status: meta.status,
        code,
        message,
        details,
        param,
        request_id: meta.request_id,
        retry_after: headers.get(RETRY_AFTER).and_then(retry_after),
        headers,
        body,
    }
}

fn retry_after(value: &HeaderValue) -> Option<Duration> {
    let text = value.to_str().ok()?.trim();
    if let Ok(seconds) = text.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }
    let date = httpdate::parse_http_date(text).ok()?;
    Some(
        date.duration_since(SystemTime::now())
            .unwrap_or(Duration::ZERO),
    )
}

fn redact_headers(headers: HeaderMap, sensitive: &[String]) -> HeaderMap {
    if sensitive.is_empty() {
        return headers;
    }
    headers
        .iter()
        .map(|(name, value)| {
            let redacted = value
                .to_str()
                .ok()
                .map(|text| redact_text(text, sensitive))
                .and_then(|text| HeaderValue::from_str(&text).ok())
                .unwrap_or_else(|| HeaderValue::from_static("[REDACTED]"));
            (name.clone(), redacted)
        })
        .collect()
}

fn redact_text(text: &str, sensitive: &[String]) -> String {
    let mut text = text.to_owned();
    for secret in sensitive {
        if secret.len() >= MIN_SUBSTRING_SECRET_LENGTH {
            text = text.replace(secret.as_str(), "[REDACTED]");
        } else if text == *secret {
            return "[REDACTED]".to_owned();
        }
    }
    text
}

fn redact_value(value: Value, sensitive: &[String]) -> Value {
    match value {
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(key, value)| (key, redact_value(value, sensitive)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| redact_value(item, sensitive))
                .collect(),
        ),
        Value::String(text) => Value::String(redact_text(&text, sensitive)),
        other => other,
    }
}

fn contains_credential_material(value: &Value, sensitive: &[String]) -> bool {
    match value {
        Value::Object(fields) => fields.iter().any(|(key, value)| {
            CREDENTIAL_FIELDS.contains(&key.as_str())
                || contains_credential_material(value, sensitive)
        }),
        Value::Array(items) => items
            .iter()
            .any(|item| contains_credential_material(item, sensitive)),
        Value::String(text) => sensitive.iter().any(|secret| {
            text == secret
                || (secret.len() >= MIN_SUBSTRING_SECRET_LENGTH && text.contains(secret.as_str()))
        }),
        _ => false,
    }
}

impl Raw {
    pub(crate) async fn discard(self) -> Result<()> {
        Ok(())
    }

    pub(crate) async fn bytes(self) -> Result<Bytes> {
        let meta = self.meta;
        self.response
            .bytes()
            .await
            .map_err(|error| read_error(meta, error))
    }

    pub(crate) async fn json<T: DeserializeOwned>(self) -> Result<T> {
        let operation_id = self.operation_id;
        let request_id = self.meta.request_id.clone();
        let bytes = self.bytes().await?;
        serde_json::from_slice(&bytes).map_err(|source| Error::Decode {
            operation_id,
            request_id,
            source,
        })
    }

    /// Decodes JSON after checking that the body carries no credential field and
    /// echoes no submitted secret.
    pub(crate) async fn credential_safe_json<T: DeserializeOwned>(self) -> Result<T> {
        let operation_id = self.operation_id;
        let request_id = self.meta.request_id.clone();
        let sensitive = self.sensitive.clone();
        let value: Value = self.json().await?;
        if contains_credential_material(&value, &sensitive) {
            return Err(Error::CredentialEcho {
                operation_id,
                request_id,
            });
        }
        serde_json::from_value(value).map_err(|source| Error::Decode {
            operation_id,
            request_id,
            source,
        })
    }

    pub(crate) async fn download(self) -> Result<Download> {
        Ok(Download::new(
            self.meta,
            Box::pin(self.response.bytes_stream()),
        ))
    }
}

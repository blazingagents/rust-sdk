//! A prepared request. Every SDK method returns a [`Call`]; `.await` sends it.

use std::fmt;
use std::future::{Future, IntoFuture};
use std::pin::Pin;
use std::time::Duration;

use futures_util::{TryStreamExt, stream};
use http::{HeaderMap, HeaderName, HeaderValue};
use serde::Serialize;

use crate::client::{Client, Scope, Tenant};
use crate::error::Result;
use crate::http::{Handle, RequestOptions};
use crate::op::{Op, TenantOp};
use crate::pagination::{Cursor, Items, Pages, Paginated};

/// A prepared request. Send it with `.await`, or with
/// [`send_with_meta`](Self::send_with_meta) to also get the response status,
/// headers and server request ID. It owns a handle to the client, so it is
/// `Send + 'static` and can be passed to `tokio::spawn`.
///
/// Methods prefixed `with_` set transport options and exist on every call.
/// Other methods are the operation's own optional inputs.
#[must_use = "a Call does nothing until awaited"]
pub struct Call<R: Request> {
    pub(crate) handle: Handle,
    pub(crate) route: Result<Route, InvalidSegment>,
    pub(crate) request: R,
    pub(crate) options: RequestOptions,
}

#[derive(Clone, Debug)]
pub(crate) struct Route {
    pub(crate) op_id: &'static str,
    pub(crate) method: &'static str,
    pub(crate) path: String,
}

/// The name of a path parameter whose value was empty, `.` or `..`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct InvalidSegment(pub(crate) &'static str);

impl<R: Request> Call<R> {
    /// A call any client may send. Only an end-user [`Op`] fits, so a method in a
    /// generic `impl<S: Scope>` block cannot send a tenant-only operation.
    pub(crate) fn new<S: Scope>(client: &Client<S>, op: Op<'_>, request: R) -> Self {
        Self::with_handle(client.handle().clone(), op, request)
    }

    /// A call only a `Client<Tenant>` may send. A [`TenantOp`] has no other way
    /// in, so a `UserClient` cannot reach a tenant-only operation.
    pub(crate) fn tenant(client: &Client<Tenant>, op: TenantOp<'_>, request: R) -> Self {
        Self::routed(client.handle().clone(), op, request)
    }

    pub(crate) fn with_handle(handle: Handle, op: Op<'_>, request: R) -> Self {
        Self::routed(handle, op, request)
    }

    fn routed(handle: Handle, op: impl Operation, request: R) -> Self {
        Self {
            handle,
            route: route(op),
            request,
            options: RequestOptions::default(),
        }
    }

    /// Sends a different operation with the same request state.
    pub(crate) fn reroute(mut self, op: Op<'_>) -> Self {
        self.route = route(op);
        self
    }

    /// Replaces the transport options, for a follow-up request that inherits
    /// its parent call's `with_` settings.
    pub(crate) fn with_options(mut self, options: RequestOptions) -> Self {
        self.options = options;
        self
    }

    /// Edits the request state. Setters in resource modules use this.
    pub(crate) fn map(mut self, edit: impl FnOnce(&mut R)) -> Self {
        edit(&mut self.request);
        self
    }

    /// Replaces the client's timeout for this call. For a streaming response the
    /// timeout covers the wait for headers only.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.options.timeout = Some(Some(timeout));
        self
    }

    /// Disables the timeout for this call.
    pub fn without_timeout(mut self) -> Self {
        self.options.timeout = Some(None);
        self
    }

    /// Adds a header to this call. `Authorization`, `Content-Type`,
    /// `X-Request-Id` and `X-BA-User-Id` are owned by the SDK and ignored here.
    pub fn with_header(mut self, name: HeaderName, value: HeaderValue) -> Self {
        self.options.headers.append(name, value);
        self
    }

    /// Sends `X-Client-Request-Id` on this call, overriding the client's value.
    pub fn with_client_request_id(mut self, id: impl Into<String>) -> Self {
        self.options.client_request_id = Some(id.into());
        self
    }

    /// Sends the request and returns the decoded value with the response status,
    /// headers and server request ID.
    pub async fn send_with_meta(self) -> Result<WithMeta<R::Output>> {
        crate::http::send(self).await
    }
}

impl<R: Paginate> Call<R>
where
    R::Output: Paginated,
{
    /// Starts from a saved cursor, for example to resume an iteration.
    pub fn cursor(self, cursor: Cursor) -> Self {
        self.map(|request| request.set_cursor(cursor))
    }

    /// Every page, fetched lazily. Nothing is sent before the first poll, and the
    /// stream ends after a page with no next cursor.
    pub fn pages(self) -> Pages<R::Output> {
        Pages::new(Box::pin(stream::try_unfold(
            Some(self),
            |next| async move {
                let Some(call) = next else {
                    return Ok(None);
                };
                let template = call.clone();
                let page = call.await?;
                let next = page
                    .next_cursor()
                    .cloned()
                    .map(|cursor| template.cursor(cursor));
                Ok(Some((page, next)))
            },
        )))
    }

    /// Every item across every page, fetched lazily.
    pub fn items(self) -> Items<<R::Output as Paginated>::Item> {
        Items::new(Box::pin(
            self.pages()
                .map_ok(|page| stream::iter(page.into_items().into_iter().map(Ok)))
                .try_flatten(),
        ))
    }
}

impl<R: Request + Clone> Clone for Call<R> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            route: self.route.clone(),
            request: self.request.clone(),
            options: self.options.clone(),
        }
    }
}

/// Leaves the request out: request bodies can hold credentials.
impl<R: Request> fmt::Debug for Call<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Call")
            .field("route", &self.route)
            .finish_non_exhaustive()
    }
}

impl<R: Request> IntoFuture for Call<R> {
    type Output = Result<R::Output>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move { self.send_with_meta().await.map(|with_meta| with_meta.value) })
    }
}

fn route(op: impl Operation) -> Result<Route, InvalidSegment> {
    Ok(Route {
        op_id: op.id(),
        method: op.method(),
        path: op.path().map_err(InvalidSegment)?,
    })
}

/// A generated operation: an [`Op`] or a [`TenantOp`].
pub(crate) trait Operation: Copy {
    /// The spec's `operationId`.
    fn id(self) -> &'static str;

    fn method(self) -> &'static str;

    /// The request path with every parameter percent-encoded as one segment.
    /// Returns the parameter's name when its value is empty, `.` or `..`, because
    /// URL normalization would change which endpoint receives the request.
    fn path(self) -> Result<String, &'static str>;
}

/// A decoded response with its status, headers and server request ID.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct WithMeta<T> {
    /// The decoded value.
    pub value: T,
    /// Status, headers and request ID of the response.
    pub meta: ResponseMeta,
}

/// Status, headers and server request ID of a response.
#[derive(Clone, Debug)]
pub struct ResponseMeta {
    pub(crate) status: u16,
    pub(crate) headers: HeaderMap,
    pub(crate) request_id: Option<String>,
}

impl ResponseMeta {
    /// The HTTP status.
    pub fn status(&self) -> u16 {
        self.status
    }

    /// The response headers.
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// The server request ID from `X-Request-Id`. Keep it when you contact support.
    pub fn request_id(&self) -> Option<&str> {
        self.request_id.as_deref()
    }
}

/// Implemented by each operation's request type. Sealed: callers name these
/// types (`Call<prompts::CreatePrompt>`) but cannot implement the trait.
pub trait Request: sealed::Sealed + Send + 'static {
    /// What `.await` returns.
    type Output: Send + 'static;

    /// What `encode` keeps for `decode`: `()` for most requests, the handlers
    /// and IDs a chat stream needs for the rest.
    #[doc(hidden)]
    type State: Send + 'static;

    #[doc(hidden)]
    const STREAMING: bool = false;

    #[doc(hidden)]
    fn sensitive(&self) -> Vec<String> {
        Vec::new()
    }

    #[doc(hidden)]
    fn encode(self) -> Result<(Payload, Self::State)>;

    #[doc(hidden)]
    fn decode(state: Self::State, raw: Raw) -> impl Future<Output = Result<Self::Output>> + Send;
}

/// A list request: [`cursor`](Call::cursor), [`pages`](Call::pages) and
/// [`items`](Call::items) apply to its [`Call`].
pub trait Paginate: Request + Clone
where
    Self::Output: Paginated,
{
    #[doc(hidden)]
    fn set_cursor(&mut self, cursor: Cursor);
}

pub(crate) mod sealed {
    pub trait Sealed {}
}

/// The encoded request: query pairs, extra headers and a body.
#[doc(hidden)]
#[derive(Default)]
pub struct Payload {
    pub(crate) query: Vec<(String, String)>,
    pub(crate) headers: HeaderMap,
    pub(crate) body: Body,
}

#[derive(Default)]
pub(crate) enum Body {
    #[default]
    Empty,
    Json(Vec<u8>),
    Multipart(reqwest::multipart::Form),
    OctetStream(reqwest::Body),
}

impl Payload {
    /// Serializes `value` as the JSON body.
    pub(crate) fn json(mut self, value: &impl Serialize) -> Result<Self> {
        self.body = Body::Json(serde_json::to_vec(value).map_err(body_not_serializable)?);
        Ok(self)
    }

    /// Adds `value`'s fields as query parameters. `None` fields are left out.
    pub(crate) fn query(self, value: &impl Serialize) -> Result<Self> {
        self.query_value(serde_json::to_value(value).map_err(query_not_serializable)?)
    }

    fn query_value(mut self, value: serde_json::Value) -> Result<Self> {
        let serde_json::Value::Object(fields) = value else {
            return Err(crate::Error::InvalidArgument(
                "query must serialize to an object".to_owned(),
            ));
        };
        for (name, value) in fields {
            let text = match value {
                serde_json::Value::Null => continue,
                serde_json::Value::String(text) => text,
                serde_json::Value::Bool(flag) => flag.to_string(),
                serde_json::Value::Number(number) => number.to_string(),
                other => {
                    return Err(crate::Error::InvalidArgument(format!(
                        "query parameter {name} is not a scalar: {other}"
                    )));
                }
            };
            self.query.push((name, text));
        }
        Ok(self)
    }

    pub(crate) fn header(mut self, name: HeaderName, value: HeaderValue) -> Self {
        self.headers.append(name, value);
        self
    }

    pub(crate) fn multipart(mut self, form: reqwest::multipart::Form) -> Self {
        self.body = Body::Multipart(form);
        self
    }

    pub(crate) fn octet_stream(mut self, body: reqwest::Body) -> Self {
        self.body = Body::OctetStream(body);
        self
    }
}

fn body_not_serializable(error: serde_json::Error) -> crate::Error {
    crate::Error::InvalidArgument(format!("request body is not serializable: {error}"))
}

fn query_not_serializable(error: serde_json::Error) -> crate::Error {
    crate::Error::InvalidArgument(format!("query is not serializable: {error}"))
}

/// A successful response before decoding.
#[doc(hidden)]
pub struct Raw {
    pub(crate) response: reqwest::Response,
    pub(crate) meta: ResponseMeta,
    pub(crate) sensitive: Vec<String>,
    pub(crate) operation_id: &'static str,
    pub(crate) handle: Handle,
    pub(crate) options: RequestOptions,
}

/// Implements [`Request`] for an operation's request type.
///
/// ```ignore
/// impl_request!(GetPrompt => Prompt; encode = none; decode = json);
/// impl_request!(ListPrompts => Page<Prompt>; encode = query; decode = json; paginate);
/// impl_request!(ReadSkillFile => Download; encode = query; decode = download);
/// impl_request!(UploadSkill => Skill; encode = (Self::into_payload); decode = json);
/// impl_request!(CreateProvider => Provider; encode = json; decode = credential_safe_json;
///     sensitive = (|r: &Self| vec![r.api_key.clone()]));
/// ```
///
/// `encode` is `none`, `query` (the struct's fields as query parameters), `json`
/// (the struct as the JSON body), or a parenthesized `fn(Self) -> Result<Payload>`.
/// `decode` is `none` (`()`), `json`, `credential_safe_json` (fails if the body
/// echoes credential material), `download` (a streamed body), or a parenthesized
/// `fn(Raw) -> impl Future<Output = Result<Output>>`. `paginate` implements
/// [`Paginate`] through a `cursor: Option<Cursor>` field.
macro_rules! impl_request {
    ($ty:ident => $out:ty; encode = $enc:tt; decode = $dec:tt) => {
        $crate::call::impl_request!(@impl $ty => $out; encode = $enc; decode = $dec;
            sensitive = (|_request: &Self| Vec::new()));
    };
    ($ty:ident => $out:ty; encode = $enc:tt; decode = $dec:tt; paginate) => {
        $crate::call::impl_request!($ty => $out; encode = $enc; decode = $dec);

        impl $crate::call::Paginate for $ty {
            fn set_cursor(&mut self, cursor: $crate::pagination::Cursor) {
                self.cursor = Some(cursor);
            }
        }
    };
    ($ty:ident => $out:ty; encode = $enc:tt; decode = $dec:tt; sensitive = $sens:tt) => {
        $crate::call::impl_request!(@impl $ty => $out; encode = $enc; decode = $dec; sensitive = $sens);
    };
    (@impl $ty:ident => $out:ty; encode = $enc:tt; decode = $dec:tt; sensitive = $sens:tt) => {
        impl $crate::call::sealed::Sealed for $ty {}

        impl $crate::call::Request for $ty {
            type Output = $out;
            type State = ();

            const STREAMING: bool = $crate::call::impl_request!(@streaming $dec);

            fn sensitive(&self) -> Vec<String> {
                ($sens)(self)
            }

            fn encode(self) -> $crate::error::Result<($crate::call::Payload, ())> {
                Ok(($crate::call::impl_request!(@encode $enc, self)?, ()))
            }

            fn decode(
                (): (),
                raw: $crate::call::Raw,
            ) -> impl ::core::future::Future<Output = $crate::error::Result<$out>> + Send {
                $crate::call::impl_request!(@decode $dec, raw)
            }
        }
    };
    (@streaming download) => {
        true
    };
    (@streaming $other:tt) => {
        false
    };
    (@encode none, $request:ident) => {{
        let _ = $request;
        Ok::<$crate::call::Payload, $crate::error::Error>($crate::call::Payload::default())
    }};
    (@encode json, $request:ident) => {
        $crate::call::Payload::default().json(&$request)
    };
    (@encode query, $request:ident) => {
        $crate::call::Payload::default().query(&$request)
    };
    (@encode ($encode:expr), $request:ident) => {
        ($encode)($request)
    };
    (@decode none, $raw:ident) => {
        $raw.discard()
    };
    (@decode json, $raw:ident) => {
        $raw.json()
    };
    (@decode credential_safe_json, $raw:ident) => {
        $raw.credential_safe_json()
    };
    (@decode download, $raw:ident) => {
        $raw.download()
    };
    (@decode ($decode:expr), $raw:ident) => {
        ($decode)($raw)
    };
}

pub(crate) use impl_request;

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::Payload;
    use crate::Error;

    fn invalid_argument(result: Result<Payload, Error>) -> String {
        result
            .err()
            .unwrap()
            .to_string()
            .strip_prefix("invalid argument: ")
            .unwrap()
            .to_owned()
    }

    #[test]
    fn query_pairs_come_out_in_field_order_with_nulls_dropped() {
        let payload = Payload::default()
            .query(&json!({
                "a_flag": true,
                "b_count": 3,
                "c_name": "x",
                "d_none": null
            }))
            .unwrap();

        assert_eq!(
            payload.query,
            [
                ("a_flag".to_owned(), "true".to_owned()),
                ("b_count".to_owned(), "3".to_owned()),
                ("c_name".to_owned(), "x".to_owned()),
            ]
        );
    }

    #[test]
    fn query_rejects_non_objects_and_non_scalar_fields() {
        for value in [json!("a string"), json!(["an", "array"])] {
            let message = invalid_argument(Payload::default().query(&value));
            assert_eq!(message, "query must serialize to an object");
        }
        let message = invalid_argument(Payload::default().query(&json!({ "a": [1] })));
        assert_eq!(message, "query parameter a is not a scalar: [1]");
    }

    #[test]
    fn unserializable_values_fail_the_encode() {
        let unserializable = BTreeMap::from([((1, 2), 3)]);

        let message = invalid_argument(Payload::default().query(&unserializable));
        assert!(
            message.starts_with("query is not serializable: "),
            "{message}"
        );
        let message = invalid_argument(Payload::default().json(&unserializable));
        assert!(
            message.starts_with("request body is not serializable: "),
            "{message}"
        );
    }
}

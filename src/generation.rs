//! Stateless generation: text with [`Client::completion`] and
//! [`Client::completion_stream`], structured output with [`Client::object`] and
//! [`Client::object_stream`]. Each call runs one metered turn.

use std::collections::BTreeMap;
use std::fmt;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use bytes::Bytes;
use futures_core::Stream;
use futures_util::StreamExt;
use serde::de::DeserializeOwned;
use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

use crate::call::{Call, Payload, Raw, Request, ResponseMeta, sealed};
use crate::client::{Client, Scope};
use crate::error::{ObjectError, ObjectErrorKind, Result, StreamError, StreamErrorKind};
use crate::ids::{AgentId, PromptId};
use crate::op::Op;
use crate::types::Metadata;

/// The prompt of a generation: literal text, or a saved prompt with variables.
#[derive(Clone, Debug, PartialEq)]
pub enum PromptInput {
    /// Literal prompt text.
    Text(String),
    /// A saved prompt and its variable values.
    Saved(SavedPrompt),
}

impl From<&str> for PromptInput {
    fn from(text: &str) -> Self {
        Self::Text(text.to_owned())
    }
}

impl From<String> for PromptInput {
    fn from(text: String) -> Self {
        Self::Text(text)
    }
}

impl From<SavedPrompt> for PromptInput {
    fn from(prompt: SavedPrompt) -> Self {
        Self::Saved(prompt)
    }
}

impl Serialize for PromptInput {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        match self {
            PromptInput::Text(text) => map.serialize_entry("prompt", text)?,
            PromptInput::Saved(prompt) => prompt.serialize_fields(&mut map)?,
        }
        map.end()
    }
}

/// A saved prompt to run, with values for its variables. Supply exactly the
/// names the prompt's `variables` lists.
#[derive(Clone, Debug, PartialEq)]
pub struct SavedPrompt {
    id: PromptId,
    variables: BTreeMap<String, String>,
}

impl SavedPrompt {
    /// Runs the prompt with the given ID.
    pub fn new(id: impl Into<PromptId>) -> Self {
        Self {
            id: id.into(),
            variables: BTreeMap::new(),
        }
    }

    /// Sets one variable.
    #[must_use]
    pub fn var(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.variables.insert(name.into(), value.into());
        self
    }

    /// The prompt's ID.
    pub fn id(&self) -> &PromptId {
        &self.id
    }

    /// The variable values.
    pub fn variables(&self) -> &BTreeMap<String, String> {
        &self.variables
    }

    pub(crate) fn serialize_fields<M: SerializeMap>(
        &self,
        map: &mut M,
    ) -> std::result::Result<(), M::Error> {
        map.serialize_entry("promptId", &self.id)?;
        if !self.variables.is_empty() {
            map.serialize_entry("variables", &self.variables)?;
        }
        Ok(())
    }
}

/// The JSON Schema an object generation must satisfy, and the type the final
/// JSON is decoded into.
pub struct ObjectSchema<T> {
    schema: serde_json::Value,
    _output: PhantomData<fn() -> T>,
}

#[cfg(feature = "schemars")]
impl<T: DeserializeOwned + schemars::JsonSchema> ObjectSchema<T> {
    /// Derives the schema from `T`.
    pub fn derive() -> Self {
        let schema = serde_json::to_value(schemars::schema_for!(T))
            .expect("a JSON Schema serializes to JSON");
        Self::from_json_schema(schema)
    }
}

impl<T: DeserializeOwned> ObjectSchema<T> {
    /// Uses a hand-written JSON Schema. With `T = serde_json::Value` the final
    /// JSON is returned without further validation.
    pub fn from_json_schema(schema: serde_json::Value) -> Self {
        Self {
            schema,
            _output: PhantomData,
        }
    }

    /// The JSON Schema.
    pub fn schema(&self) -> &serde_json::Value {
        &self.schema
    }
}

impl<T> Clone for ObjectSchema<T> {
    fn clone(&self) -> Self {
        Self {
            schema: self.schema.clone(),
            _output: PhantomData,
        }
    }
}

impl<T> fmt::Debug for ObjectSchema<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObjectSchema")
            .field("schema", &self.schema)
            .finish()
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Output {
    Text,
    Object { schema: serde_json::Value },
}

/// The body of every generation request.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Generate {
    #[serde(flatten)]
    prompt: PromptInput,
    output: Output,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<Metadata>,
}

impl Generate {
    fn new(prompt: PromptInput, output: Output) -> Self {
        Self {
            prompt,
            output,
            user_id: None,
            metadata: None,
        }
    }
}

impl<S: Scope> Client<S> {
    /// Generates the complete text answer to `prompt`.
    pub fn completion(
        &self,
        agent_id: impl Into<AgentId>,
        prompt: impl Into<PromptInput>,
    ) -> Call<Completion> {
        let agent_id = agent_id.into();
        Call::new(
            self,
            Op::Generate {
                agent_id: agent_id.as_str(),
            },
            Completion(Generate::new(prompt.into(), Output::Text)),
        )
    }

    /// Streams the text answer to `prompt` as it is generated.
    pub fn completion_stream(
        &self,
        agent_id: impl Into<AgentId>,
        prompt: impl Into<PromptInput>,
    ) -> Call<CompletionStreaming> {
        let agent_id = agent_id.into();
        Call::new(
            self,
            Op::Generate {
                agent_id: agent_id.as_str(),
            },
            CompletionStreaming(Generate::new(prompt.into(), Output::Text)),
        )
    }

    /// Generates JSON that matches `schema` and decodes it into `T`.
    pub fn object<T: DeserializeOwned + Send + 'static>(
        &self,
        agent_id: impl Into<AgentId>,
        prompt: impl Into<PromptInput>,
        schema: ObjectSchema<T>,
    ) -> Call<Object<T>> {
        let agent_id = agent_id.into();
        Call::new(
            self,
            Op::Generate {
                agent_id: agent_id.as_str(),
            },
            Object {
                generate: Generate::new(
                    prompt.into(),
                    Output::Object {
                        schema: schema.schema,
                    },
                ),
                _output: PhantomData,
            },
        )
    }

    /// Streams the JSON text of an object generation and decodes it into `T` at
    /// the end.
    pub fn object_stream<T: DeserializeOwned + Send + 'static>(
        &self,
        agent_id: impl Into<AgentId>,
        prompt: impl Into<PromptInput>,
        schema: ObjectSchema<T>,
    ) -> Call<ObjectStreaming<T>> {
        let agent_id = agent_id.into();
        Call::new(
            self,
            Op::Generate {
                agent_id: agent_id.as_str(),
            },
            ObjectStreaming {
                generate: Generate::new(
                    prompt.into(),
                    Output::Object {
                        schema: schema.schema,
                    },
                ),
                _output: PhantomData,
            },
        )
    }
}

macro_rules! generation_setters {
    ([$($generics:tt)*] $ty:ty) => {
        impl<$($generics)*> Call<$ty>
        where
            $ty: Request,
        {
            /// Attributes the turn to an end user.
            pub fn user_id(self, user_id: impl Into<String>) -> Self {
                self.map(|request| request.generate_mut().user_id = Some(user_id.into()))
            }

            /// Your own data, stored with the turn.
            pub fn metadata(self, metadata: Metadata) -> Self {
                self.map(|request| request.generate_mut().metadata = Some(metadata))
            }
        }
    };
}

generation_setters!([] Completion);
generation_setters!([] CompletionStreaming);
generation_setters!([T: DeserializeOwned + Send + 'static] Object<T>);
generation_setters!([T: DeserializeOwned + Send + 'static] ObjectStreaming<T>);

/// Request state of [`Client::completion`].
#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct Completion(Generate);

impl Completion {
    fn generate_mut(&mut self) -> &mut Generate {
        &mut self.0
    }
}

impl sealed::Sealed for Completion {}

impl Request for Completion {
    type Output = String;
    type State = ();

    /// The timeout bounds the wait for headers only; a long generation then
    /// streams for as long as it takes.
    const STREAMING: bool = true;

    fn encode(self) -> Result<(Payload, ())> {
        Ok((Payload::default().json(&self)?, ()))
    }

    async fn decode((): (), raw: Raw) -> Result<String> {
        CompletionStream::open(raw).final_text().await
    }
}

/// Request state of [`Client::completion_stream`].
#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct CompletionStreaming(Generate);

impl CompletionStreaming {
    fn generate_mut(&mut self) -> &mut Generate {
        &mut self.0
    }
}

impl sealed::Sealed for CompletionStreaming {}

impl Request for CompletionStreaming {
    type Output = CompletionStream;
    type State = ();

    const STREAMING: bool = true;

    fn encode(self) -> Result<(Payload, ())> {
        Ok((Payload::default().json(&self)?, ()))
    }

    async fn decode((): (), raw: Raw) -> Result<CompletionStream> {
        Ok(CompletionStream::open(raw))
    }
}

/// Request state of [`Client::object`].
#[derive(Clone, Debug)]
pub struct Object<T> {
    generate: Generate,
    _output: PhantomData<fn() -> T>,
}

impl<T> Object<T> {
    fn generate_mut(&mut self) -> &mut Generate {
        &mut self.generate
    }
}

impl<T> sealed::Sealed for Object<T> {}

impl<T: DeserializeOwned + Send + 'static> Request for Object<T> {
    type Output = T;
    type State = ();

    /// The timeout bounds the wait for headers only; a long generation then
    /// streams for as long as it takes.
    const STREAMING: bool = true;

    fn encode(self) -> Result<(Payload, ())> {
        Ok((Payload::default().json(&self.generate)?, ()))
    }

    async fn decode((): (), raw: Raw) -> Result<T> {
        ObjectStream::<T>::open(raw).final_object().await
    }
}

/// Request state of [`Client::object_stream`].
#[derive(Clone, Debug)]
pub struct ObjectStreaming<T> {
    generate: Generate,
    _output: PhantomData<fn() -> T>,
}

impl<T> ObjectStreaming<T> {
    fn generate_mut(&mut self) -> &mut Generate {
        &mut self.generate
    }
}

impl<T> sealed::Sealed for ObjectStreaming<T> {}

impl<T: DeserializeOwned + Send + 'static> Request for ObjectStreaming<T> {
    type Output = ObjectStream<T>;
    type State = ();

    const STREAMING: bool = true;

    fn encode(self) -> Result<(Payload, ())> {
        Ok((Payload::default().json(&self.generate)?, ()))
    }

    async fn decode((): (), raw: Raw) -> Result<ObjectStream<T>> {
        Ok(ObjectStream::open(raw))
    }
}

type Body = Pin<Box<dyn Stream<Item = reqwest::Result<Bytes>> + Send>>;

enum BodyState {
    Reading(Body),
    /// The body ended and every byte was decoded.
    Complete,
    /// An error of this kind was already yielded.
    Failed(StreamErrorKind),
}

/// A response body decoded as UTF-8 text deltas, with every delta kept for
/// the final text. Shared by the completion and object streams and by their
/// buffered forms.
struct TextDeltas {
    state: BodyState,
    /// The bytes of a character split across chunk boundaries.
    pending: Vec<u8>,
    text: String,
    meta: ResponseMeta,
}

impl TextDeltas {
    fn open(raw: Raw) -> Self {
        Self {
            state: BodyState::Reading(Box::pin(raw.response.bytes_stream())),
            pending: Vec::new(),
            text: String::new(),
            meta: raw.meta,
        }
    }

    #[cfg(test)]
    fn from_chunks(
        chunks: impl Stream<Item = reqwest::Result<Bytes>> + Send + 'static,
        meta: ResponseMeta,
    ) -> Self {
        Self {
            state: BodyState::Reading(Box::pin(chunks)),
            pending: Vec::new(),
            text: String::new(),
            meta,
        }
    }

    /// Decodes `chunk` after the pending bytes. An invalid sequence becomes
    /// U+FFFD; an incomplete trailing sequence waits for the next chunk.
    fn decode(&mut self, chunk: &[u8]) -> String {
        let mut bytes = std::mem::take(&mut self.pending);
        bytes.extend_from_slice(chunk);
        let mut delta = String::new();
        let mut rest = bytes.as_slice();
        loop {
            match std::str::from_utf8(rest) {
                Ok(valid) => {
                    delta.push_str(valid);
                    break;
                }
                Err(error) => {
                    let (valid, invalid) = rest.split_at(error.valid_up_to());
                    delta.push_str(std::str::from_utf8(valid).expect("validated prefix"));
                    match error.error_len() {
                        None => {
                            self.pending = invalid.to_vec();
                            break;
                        }
                        Some(skipped) => {
                            delta.push(char::REPLACEMENT_CHARACTER);
                            rest = &invalid[skipped..];
                        }
                    }
                }
            }
        }
        delta
    }

    fn fail(&mut self, kind: StreamErrorKind, source: Option<reqwest::Error>) -> crate::Error {
        self.state = BodyState::Failed(kind);
        StreamError {
            kind,
            meta: self.meta.clone(),
            source: source.map(|error| Box::new(error.without_url()) as _),
        }
        .into()
    }

    fn poll_delta(&mut self, cx: &mut Context<'_>) -> Poll<Option<Result<String>>> {
        loop {
            let BodyState::Reading(body) = &mut self.state else {
                return Poll::Ready(None);
            };
            match ready!(body.as_mut().poll_next(cx)) {
                Some(Ok(chunk)) => {
                    let delta = self.decode(&chunk);
                    if delta.is_empty() {
                        continue;
                    }
                    self.text.push_str(&delta);
                    return Poll::Ready(Some(Ok(delta)));
                }
                Some(Err(error)) => {
                    return Poll::Ready(Some(Err(self.fail(StreamErrorKind::Read, Some(error)))));
                }
                None if self.pending.is_empty() => {
                    self.state = BodyState::Complete;
                    return Poll::Ready(None);
                }
                None => {
                    return Poll::Ready(Some(Err(self.fail(StreamErrorKind::Incomplete, None))));
                }
            }
        }
    }

    /// Reads whatever has not been consumed and returns the full text.
    async fn finish(mut self) -> Result<String> {
        while let Some(delta) = self.next().await {
            delta?;
        }
        if let BodyState::Failed(kind) = self.state {
            return Err(self.fail(kind, None));
        }
        Ok(self.text)
    }
}

impl Stream for TextDeltas {
    type Item = Result<String>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.get_mut().poll_delta(cx)
    }
}

/// Text deltas of a completion, decoded as UTF-8 across chunk boundaries. One
/// consumer, by ownership. Drop it to close the connection.
pub struct CompletionStream {
    deltas: TextDeltas,
}

impl CompletionStream {
    fn open(raw: Raw) -> Self {
        Self {
            deltas: TextDeltas::open(raw),
        }
    }

    /// Status, headers and request ID of the response.
    pub fn meta(&self) -> &ResponseMeta {
        &self.deltas.meta
    }

    /// Reads whatever has not been consumed and returns the full text. Fails
    /// if the body did not end cleanly.
    pub async fn final_text(self) -> Result<String> {
        self.deltas.finish().await
    }
}

impl Stream for CompletionStream {
    type Item = Result<String>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.get_mut().deltas.poll_delta(cx)
    }
}

impl fmt::Debug for CompletionStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompletionStream")
            .field("meta", &self.deltas.meta)
            .finish_non_exhaustive()
    }
}

/// Raw JSON text deltas of an object generation. Nothing is validated until
/// [`final_object`](Self::final_object).
pub struct ObjectStream<T> {
    deltas: TextDeltas,
    _output: PhantomData<fn() -> T>,
}

impl<T: DeserializeOwned> ObjectStream<T> {
    fn open(raw: Raw) -> Self {
        Self {
            deltas: TextDeltas::open(raw),
            _output: PhantomData,
        }
    }

    /// Status, headers and request ID of the response.
    pub fn meta(&self) -> &ResponseMeta {
        &self.deltas.meta
    }

    /// Reads whatever has not been consumed, then decodes the complete text
    /// into `T`. Truncated and invalid JSON and a type mismatch are told apart
    /// by the [`ObjectError`]'s kind.
    pub async fn final_object(self) -> Result<T> {
        let meta = self.deltas.meta.clone();
        let text = self.deltas.finish().await?;
        decode_object(text, meta)
    }
}

/// Decodes generated JSON in two phases, so a truncated or invalid body and a
/// schema mismatch are told apart.
fn decode_object<T: DeserializeOwned>(text: String, meta: ResponseMeta) -> Result<T> {
    let value: serde_json::Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(source) => {
            let kind = if source.is_eof() {
                ObjectErrorKind::Truncated
            } else {
                ObjectErrorKind::InvalidJson
            };
            return Err(ObjectError {
                kind,
                text,
                meta,
                source,
            }
            .into());
        }
    };
    serde_json::from_value(value).map_err(|source| {
        ObjectError {
            kind: ObjectErrorKind::Mismatch,
            text,
            meta,
            source,
        }
        .into()
    })
}

impl<T> Stream for ObjectStream<T> {
    type Item = Result<String>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.get_mut().deltas.poll_delta(cx)
    }
}

impl<T> fmt::Debug for ObjectStream<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObjectStream")
            .field("meta", &self.deltas.meta)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use futures_util::{StreamExt, stream};
    use http::HeaderMap;

    use super::TextDeltas;
    use crate::call::ResponseMeta;
    use crate::error::{Error, StreamErrorKind};

    fn meta() -> ResponseMeta {
        ResponseMeta {
            status: 200,
            headers: HeaderMap::new(),
            request_id: None,
        }
    }

    fn deltas(chunks: Vec<reqwest::Result<Bytes>>) -> TextDeltas {
        TextDeltas::from_chunks(stream::iter(chunks), meta())
    }

    #[tokio::test]
    async fn a_character_split_across_chunks_is_decoded_once_complete() {
        let euro = "€".as_bytes();
        let mut deltas = deltas(vec![
            Ok(Bytes::from([b"price: ", &euro[..1]].concat())),
            Ok(Bytes::from([&euro[1..], b"5".as_slice()].concat())),
        ]);

        let first = deltas.next().await.unwrap().unwrap();
        let second = deltas.next().await.unwrap().unwrap();

        assert_eq!(first, "price: ");
        assert_eq!(second, "€5");
        assert!(deltas.next().await.is_none());
        assert_eq!(deltas.finish().await.unwrap(), "price: €5");
    }

    #[tokio::test]
    async fn an_invalid_sequence_becomes_the_replacement_character() {
        let mut deltas = deltas(vec![Ok(Bytes::from_static(b"a\xffb"))]);

        assert_eq!(deltas.next().await.unwrap().unwrap(), "a\u{FFFD}b");
    }

    #[tokio::test]
    async fn chunks_inside_a_character_yield_no_delta_until_it_is_whole() {
        let euro = "€".as_bytes();
        let mut deltas = deltas(vec![
            Ok(Bytes::copy_from_slice(&euro[..1])),
            Ok(Bytes::copy_from_slice(&euro[1..2])),
            Ok(Bytes::copy_from_slice(&euro[2..])),
        ]);

        assert_eq!(deltas.next().await.unwrap().unwrap(), "€");
        assert!(deltas.next().await.is_none());
        assert_eq!(deltas.finish().await.unwrap(), "€");
    }

    #[tokio::test]
    async fn a_read_error_is_reported_again_by_finish() {
        let failing = reqwest::get("http://[::1]:1/").await.unwrap_err();
        let mut deltas = deltas(vec![Ok(Bytes::from_static(b"partial")), Err(failing)]);

        assert_eq!(deltas.next().await.unwrap().unwrap(), "partial");
        let error = deltas.next().await.unwrap().unwrap_err();
        assert!(
            matches!(&error, Error::Stream(e) if e.kind() == StreamErrorKind::Read),
            "{error:?}"
        );
        let error = deltas.finish().await.unwrap_err();
        assert!(
            matches!(&error, Error::Stream(e) if e.kind() == StreamErrorKind::Read),
            "{error:?}"
        );
    }

    #[tokio::test]
    async fn a_body_ending_mid_character_is_incomplete() {
        let deltas = deltas(vec![Ok(Bytes::from_static(b"ok \xe2\x82"))]);

        let error = deltas.finish().await.unwrap_err();

        assert!(
            matches!(&error, Error::Stream(e) if e.kind() == StreamErrorKind::Incomplete),
            "{error:?}"
        );
    }
}

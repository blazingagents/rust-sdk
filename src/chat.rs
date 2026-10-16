//! Chat turns in sessions. Start from [`Client::chat`], [`Client::regenerate`]
//! and [`Client::continue_chat`]. Each returns a [`ChatStream`] of AI SDK UI
//! message stream events.

use std::fmt;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use bytes::Bytes;
use futures_core::Stream;
use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

use crate::call::{Call, Payload, Raw, Request, ResponseMeta, sealed};
use crate::client::{Client, Scope};
use crate::error::{Result, StreamError, StreamErrorKind};
use crate::functions::{Functions, Runner, is_platform_id};
use crate::generation::SavedPrompt;
use crate::ids::{AgentId, ApprovalId, SessionId, TurnId};
use crate::messages::UserMessage;
use crate::op::Op;
use crate::types::Metadata;

pub(crate) mod sse;

/// The input of a chat turn: one or more user messages, or a saved prompt.
#[derive(Clone, Debug, PartialEq)]
pub enum ChatInput {
    /// One or more user messages, each saved separately in the session, in order.
    Messages {
        /// The first message.
        first: UserMessage,
        /// Any further messages.
        rest: Vec<UserMessage>,
    },
    /// A saved prompt and its variable values.
    Saved(SavedPrompt),
}

impl From<UserMessage> for ChatInput {
    fn from(message: UserMessage) -> Self {
        Self::Messages {
            first: message,
            rest: Vec::new(),
        }
    }
}

impl From<SavedPrompt> for ChatInput {
    fn from(prompt: SavedPrompt) -> Self {
        Self::Saved(prompt)
    }
}

/// A message batch with no messages.
#[derive(Debug, thiserror::Error)]
#[error("a chat turn needs at least one message")]
pub struct EmptyBatch;

impl TryFrom<Vec<UserMessage>> for ChatInput {
    type Error = EmptyBatch;

    fn try_from(messages: Vec<UserMessage>) -> std::result::Result<Self, Self::Error> {
        let mut messages = messages.into_iter();
        let first = messages.next().ok_or(EmptyBatch)?;
        Ok(Self::Messages {
            first,
            rest: messages.collect(),
        })
    }
}

impl Serialize for ChatInput {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        match self {
            ChatInput::Messages { first, rest } => {
                let messages: Vec<&UserMessage> = std::iter::once(first).chain(rest).collect();
                map.serialize_entry("messages", &messages)?;
            }
            ChatInput::Saved(prompt) => prompt.serialize_fields(&mut map)?,
        }
        map.end()
    }
}

/// The input of a regeneration: one user message or a saved prompt.
#[derive(Clone, Debug, PartialEq)]
pub enum RegenerateInput {
    /// One user message.
    Message(UserMessage),
    /// A saved prompt and its variable values.
    Saved(SavedPrompt),
}

impl From<UserMessage> for RegenerateInput {
    fn from(message: UserMessage) -> Self {
        Self::Message(message)
    }
}

impl From<SavedPrompt> for RegenerateInput {
    fn from(prompt: SavedPrompt) -> Self {
        Self::Saved(prompt)
    }
}

impl Serialize for RegenerateInput {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        match self {
            RegenerateInput::Message(message) => {
                map.serialize_entry("messages", &[message])?;
            }
            RegenerateInput::Saved(prompt) => prompt.serialize_fields(&mut map)?,
        }
        map.end()
    }
}

/// One decision of a tool approval round.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalDecision {
    approval_id: ApprovalId,
    approved: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

impl ApprovalDecision {
    /// Approves the call.
    pub fn approve(approval_id: impl Into<ApprovalId>) -> Self {
        Self {
            approval_id: approval_id.into(),
            approved: true,
            reason: None,
        }
    }

    /// Denies the call.
    pub fn deny(approval_id: impl Into<ApprovalId>) -> Self {
        Self {
            approval_id: approval_id.into(),
            approved: false,
            reason: None,
        }
    }

    /// Explains the decision to the agent.
    #[must_use]
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl<S: Scope> Client<S> {
    /// Sends `input` in a new session, or in an existing one after
    /// [`session`](Call::session). The session ID is available on the stream
    /// before the body is read.
    pub fn chat(
        &self,
        agent_id: impl Into<AgentId>,
        input: impl Into<ChatInput>,
    ) -> Call<ChatTurn> {
        let agent_id = agent_id.into();
        Call::new(
            self,
            Op::CreateSessionTurn {
                agent_id: agent_id.as_str(),
            },
            ChatTurn {
                agent_id: agent_id.clone(),
                session_id: None,
                input: input.into(),
                trigger: "submit-message",
                user_id: None,
                metadata: None,
                functions: None,
            },
        )
    }

    /// Generates the answer again in an existing session, optionally cutting
    /// the history back to [`message_id`](Call::message_id) first.
    pub fn regenerate(
        &self,
        agent_id: impl Into<AgentId>,
        session_id: impl Into<SessionId>,
        input: impl Into<RegenerateInput>,
    ) -> Call<Regenerate> {
        let agent_id = agent_id.into();
        let session_id = session_id.into();
        Call::new(
            self,
            Op::ResumeSessionTurn {
                agent_id: agent_id.as_str(),
                session_id: session_id.as_str(),
            },
            Regenerate {
                agent_id: agent_id.clone(),
                session_id: session_id.clone(),
                input: input.into(),
                trigger: "regenerate-message",
                message_id: None,
                user_id: None,
                metadata: None,
                functions: None,
            },
        )
    }

    /// Records one complete tool approval round and streams the rest of the
    /// turn.
    pub fn continue_chat(
        &self,
        agent_id: impl Into<AgentId>,
        session_id: impl Into<SessionId>,
        decisions: impl IntoIterator<Item = ApprovalDecision>,
    ) -> Call<ContinueChat> {
        let agent_id = agent_id.into();
        let session_id = session_id.into();
        Call::new(
            self,
            Op::ContinueToolApprovals {
                agent_id: agent_id.as_str(),
                session_id: session_id.as_str(),
            },
            ContinueChat {
                agent_id: agent_id.clone(),
                session_id: session_id.clone(),
                decisions: decisions.into_iter().collect(),
                functions: None,
            },
        )
    }
}

/// What a chat stream needs after the response arrives.
#[doc(hidden)]
pub struct ChatState {
    agent_id: AgentId,
    session_id: Option<SessionId>,
    functions: Option<Functions>,
}

/// Request state of [`Client::chat`].
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatTurn {
    #[serde(skip)]
    agent_id: AgentId,
    #[serde(skip)]
    session_id: Option<SessionId>,
    #[serde(flatten)]
    input: ChatInput,
    trigger: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<Metadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    functions: Option<Functions>,
}

impl Call<ChatTurn> {
    /// Continues an existing session instead of starting one.
    pub fn session(self, session_id: impl Into<SessionId>) -> Self {
        let session_id = session_id.into();
        let agent_id = self.request.agent_id.clone();
        self.reroute(Op::ResumeSessionTurn {
            agent_id: agent_id.as_str(),
            session_id: session_id.as_str(),
        })
        .map(|request| request.session_id = Some(session_id))
    }

    /// Attributes the turn to an end user.
    pub fn user_id(self, user_id: impl Into<String>) -> Self {
        self.map(|request| request.user_id = Some(user_id.into()))
    }

    /// Your own data, stored with the turn.
    pub fn metadata(self, metadata: Metadata) -> Self {
        self.map(|request| request.metadata = Some(metadata))
    }

    /// Backend functions the agent may call during the turn.
    pub fn functions(self, functions: Functions) -> Self {
        self.map(|request| request.functions = Some(functions))
    }
}

impl sealed::Sealed for ChatTurn {}

impl Request for ChatTurn {
    type Output = ChatStream;
    type State = ChatState;

    const STREAMING: bool = true;

    fn encode(self) -> Result<(Payload, ChatState)> {
        let payload = Payload::default().json(&self)?;
        Ok((
            payload,
            ChatState {
                agent_id: self.agent_id,
                session_id: self.session_id,
                functions: self.functions,
            },
        ))
    }

    async fn decode(state: ChatState, raw: Raw) -> Result<ChatStream> {
        ChatStream::open(state, raw)
    }
}

/// Request state of [`Client::regenerate`].
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Regenerate {
    #[serde(skip)]
    agent_id: AgentId,
    #[serde(skip)]
    session_id: SessionId,
    #[serde(flatten)]
    input: RegenerateInput,
    trigger: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    message_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<Metadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    functions: Option<Functions>,
}

impl Call<Regenerate> {
    /// Cuts the history back to this message before regenerating.
    pub fn message_id(self, message_id: impl Into<String>) -> Self {
        self.map(|request| request.message_id = Some(message_id.into()))
    }

    /// Attributes the turn to an end user.
    pub fn user_id(self, user_id: impl Into<String>) -> Self {
        self.map(|request| request.user_id = Some(user_id.into()))
    }

    /// Your own data, stored with the turn.
    pub fn metadata(self, metadata: Metadata) -> Self {
        self.map(|request| request.metadata = Some(metadata))
    }

    /// Backend functions the agent may call during the turn.
    pub fn functions(self, functions: Functions) -> Self {
        self.map(|request| request.functions = Some(functions))
    }
}

impl sealed::Sealed for Regenerate {}

impl Request for Regenerate {
    type Output = ChatStream;
    type State = ChatState;

    const STREAMING: bool = true;

    fn encode(self) -> Result<(Payload, ChatState)> {
        let payload = Payload::default().json(&self)?;
        Ok((
            payload,
            ChatState {
                agent_id: self.agent_id,
                session_id: Some(self.session_id),
                functions: self.functions,
            },
        ))
    }

    async fn decode(state: ChatState, raw: Raw) -> Result<ChatStream> {
        ChatStream::open(state, raw)
    }
}

/// Request state of [`Client::continue_chat`].
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinueChat {
    #[serde(skip)]
    agent_id: AgentId,
    #[serde(skip)]
    session_id: SessionId,
    decisions: Vec<ApprovalDecision>,
    #[serde(skip_serializing_if = "Option::is_none")]
    functions: Option<Functions>,
}

impl Call<ContinueChat> {
    /// Backend functions the agent may call during the rest of the turn.
    pub fn functions(self, functions: Functions) -> Self {
        self.map(|request| request.functions = Some(functions))
    }
}

impl sealed::Sealed for ContinueChat {}

impl Request for ContinueChat {
    type Output = ChatStream;
    type State = ChatState;

    const STREAMING: bool = true;

    fn encode(self) -> Result<(Payload, ChatState)> {
        let payload = Payload::default().json(&self)?;
        Ok((
            payload,
            ChatState {
                agent_id: self.agent_id,
                session_id: Some(self.session_id),
                functions: self.functions,
            },
        ))
    }

    async fn decode(state: ChatState, raw: Raw) -> Result<ChatStream> {
        ChatStream::open(state, raw)
    }
}

/// An AI SDK UI message stream: each item is one complete SSE event's exact
/// bytes, ready to relay to a browser. With functions attached, the private
/// function-call events are consumed and never yielded. Drop the stream to
/// close the connection and abort running handlers.
pub struct ChatStream {
    meta: ResponseMeta,
    session_id: SessionId,
    turn_id: TurnId,
    frames: sse::Frames,
    runner: Option<Runner>,
}

impl ChatStream {
    fn open(state: ChatState, raw: Raw) -> Result<Self> {
        let meta = raw.meta;
        let session_id = match state.session_id {
            Some(session_id) => session_id,
            None => session_id_from_location(&meta)?,
        };
        let turn_id = turn_id_from_header(&meta)?;
        let runner = state.functions.map(|functions| {
            Runner::new(
                raw.handle,
                state.agent_id,
                session_id.clone(),
                functions,
                meta.clone(),
            )
        });
        Ok(Self {
            frames: sse::Frames::new(Box::pin(raw.response.bytes_stream()), meta.clone()),
            meta,
            session_id,
            turn_id,
            runner,
        })
    }

    /// The session the turn runs in.
    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    /// The running turn, for [`Sessions::stop`](crate::sessions::Sessions::stop).
    pub fn turn_id(&self) -> &TurnId {
        &self.turn_id
    }

    /// Status, headers and request ID of the response.
    pub fn meta(&self) -> &ResponseMeta {
        &self.meta
    }
}

impl Stream for ChatStream {
    type Item = Result<Bytes>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        loop {
            if let Some(runner) = &mut this.runner {
                if let Poll::Ready(Some(error)) = runner.poll_failure(cx) {
                    return Poll::Ready(Some(Err(error)));
                }
            }
            let frame = match ready!(Pin::new(&mut this.frames).poll_next(cx)) {
                None => return Poll::Ready(None),
                Some(Err(error)) => return Poll::Ready(Some(Err(error))),
                Some(Ok(frame)) => frame,
            };
            if let Some(runner) = &mut this.runner {
                match runner.intercept(&frame) {
                    Ok(true) => continue,
                    Ok(false) => {}
                    Err(error) => return Poll::Ready(Some(Err(error))),
                }
            }
            return Poll::Ready(Some(Ok(frame)));
        }
    }
}

impl fmt::Debug for ChatStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChatStream")
            .field("session_id", &self.session_id)
            .field("turn_id", &self.turn_id)
            .field("meta", &self.meta)
            .finish_non_exhaustive()
    }
}

fn session_id_from_location(meta: &ResponseMeta) -> Result<SessionId> {
    let location = meta
        .headers()
        .get(http::header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| stream_error(StreamErrorKind::MissingSessionId, meta))?;
    let candidate = location.rsplit('/').next().unwrap_or_default();
    if !is_platform_id(candidate, "ss_") {
        return Err(stream_error(StreamErrorKind::MalformedSessionId, meta));
    }
    Ok(SessionId::from(candidate))
}

fn turn_id_from_header(meta: &ResponseMeta) -> Result<TurnId> {
    let turn_id = meta
        .headers()
        .get("x-ba-turn-id")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| stream_error(StreamErrorKind::MissingTurnId, meta))?;
    if !is_platform_id(turn_id, "turn_") {
        return Err(stream_error(StreamErrorKind::MalformedTurnId, meta));
    }
    Ok(TurnId::from(turn_id))
}

fn stream_error(kind: StreamErrorKind, meta: &ResponseMeta) -> crate::Error {
    StreamError {
        kind,
        meta: meta.clone(),
        source: None,
    }
    .into()
}

//! Sessions, their transcripts, inputs and tool approvals. Start from
//! [`Client::sessions`]. Turns are sent with [`Client::chat`].
//!
//! Documentation: <https://docs.blazingagents.com/platform/sessions-and-turns>.

use http::HeaderValue;
use serde::{Deserialize, Serialize};

use crate::call::{Call, Paginate, Payload, impl_request};
use crate::client::{Client, Scope};
use crate::error::Result;
use crate::ids::{AgentId, ApprovalId, McpConnectionId, SessionId, TurnId};
use crate::messages::UserMessage;
use crate::op::Op;
use crate::pagination::{Cursor, Page, Paginated};
use crate::transcript::UiMessage;
use crate::types::{Metadata, Timestamp};

/// Session operations. Returned by [`Client::sessions`].
#[derive(Clone, Copy, Debug)]
pub struct Sessions<'a, S: Scope> {
    client: &'a Client<S>,
}

impl<S: Scope> Client<S> {
    /// Session operations.
    pub fn sessions(&self) -> Sessions<'_, S> {
        Sessions { client: self }
    }
}

impl<S: Scope> Sessions<'_, S> {
    /// Lists an agent's sessions, most recently updated first. An unknown
    /// agent returns an empty page.
    pub fn list(&self, agent_id: impl Into<AgentId>) -> Call<ListSessions> {
        let agent_id = agent_id.into();
        Call::new(
            self.client,
            Op::ListSessions {
                agent_id: agent_id.as_str(),
            },
            ListSessions::default(),
        )
    }

    /// Lists the most recently updated sessions across all your agents,
    /// including sessions of disabled agents.
    pub fn list_latest(&self) -> Call<ListLatestSessions> {
        Call::new(
            self.client,
            Op::ListLatestSessions,
            ListLatestSessions::default(),
        )
    }

    /// Gets one session and the agent configuration saved for its first turn.
    /// Later agent edits do not change it.
    pub fn get(
        &self,
        agent_id: impl Into<AgentId>,
        session_id: impl Into<SessionId>,
    ) -> Call<GetSession> {
        let agent_id = agent_id.into();
        let session_id = session_id.into();
        Call::new(
            self.client,
            Op::GetSession {
                agent_id: agent_id.as_str(),
                session_id: session_id.as_str(),
            },
            GetSession,
        )
    }

    /// Forks a session at `message_id`, which must name a saved assistant
    /// reply: one [`UiMessage::branchable`] in the transcript. The idle child
    /// copies the conversation up to and including that message and inherits
    /// the source's configuration, user label and metadata. Reuse
    /// `idempotency_key` when retrying: a replay returns the same child. The
    /// key is 1 to 200 characters.
    pub fn fork(
        &self,
        agent_id: impl Into<AgentId>,
        session_id: impl Into<SessionId>,
        message_id: impl Into<String>,
        idempotency_key: impl Into<String>,
    ) -> Call<ForkSession> {
        let agent_id = agent_id.into();
        let session_id = session_id.into();
        Call::new(
            self.client,
            Op::ForkSession {
                agent_id: agent_id.as_str(),
                session_id: session_id.as_str(),
            },
            ForkSession {
                message_id: message_id.into(),
                idempotency_key: idempotency_key.into(),
            },
        )
    }

    /// Lists a session's messages: without a position, the newest page in
    /// chronological order. Pass [`next_cursor`](MessagesPage::next_cursor) to
    /// [`.cursor()`](Call::cursor) to walk further back. `.cursor(c)` after
    /// [`.after(_)`](Call::after) sends `after=c`: the forward direction is
    /// kept, so [`.items()`](Call::items) resumed from a saved forward cursor
    /// keeps polling forward.
    ///
    /// ```no_run
    /// # async fn example(client: blazing_agents::Client) -> blazing_agents::Result<()> {
    /// let page = client
    ///     .sessions()
    ///     .messages("ag_4kP9sT2vXq7LmN3a", "ss_6Rt2Mw8KqZ4Nc1Hp")
    ///     .await?;
    /// let mut saved = page.latest_cursor;
    /// if let Some(cursor) = saved.clone() {
    ///     let newer = client
    ///         .sessions()
    ///         .messages("ag_4kP9sT2vXq7LmN3a", "ss_6Rt2Mw8KqZ4Nc1Hp")
    ///         .after(cursor)
    ///         .await?;
    ///     println!("{} new messages", newer.data.len());
    ///     // `None` means nothing new to anchor on: keep the previous cursor.
    ///     if let Some(latest) = newer.latest_cursor {
    ///         saved = Some(latest);
    ///     }
    /// }
    /// if saved.is_none() {
    ///     println!("no new messages yet");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn messages(
        &self,
        agent_id: impl Into<AgentId>,
        session_id: impl Into<SessionId>,
    ) -> Call<ListSessionMessages> {
        let agent_id = agent_id.into();
        let session_id = session_id.into();
        Call::new(
            self.client,
            Op::ListSessionMessages {
                agent_id: agent_id.as_str(),
                session_id: session_id.as_str(),
            },
            ListSessionMessages::default(),
        )
    }

    /// Lists a session's pending tool approvals. Send the round's decisions
    /// with [`Client::continue_chat`].
    pub fn tool_approvals(
        &self,
        agent_id: impl Into<AgentId>,
        session_id: impl Into<SessionId>,
    ) -> Call<ListToolApprovals> {
        let agent_id = agent_id.into();
        let session_id = session_id.into();
        Call::new(
            self.client,
            Op::ListToolApprovals {
                agent_id: agent_id.as_str(),
                session_id: session_id.as_str(),
            },
            ListToolApprovals,
        )
    }

    /// Permanently deletes a session and its transcript and, with
    /// `delete_artifacts`, its artifacts. Fails with `session_busy` while a
    /// turn is running or an approval continuation is still open.
    pub fn delete(
        &self,
        agent_id: impl Into<AgentId>,
        session_id: impl Into<SessionId>,
        delete_artifacts: bool,
    ) -> Call<DeleteSession> {
        let agent_id = agent_id.into();
        let session_id = session_id.into();
        Call::new(
            self.client,
            Op::DeleteSession {
                agent_id: agent_id.as_str(),
                session_id: session_id.as_str(),
            },
            DeleteSession { delete_artifacts },
        )
    }

    /// Steers a message into the running turn. Reuse `request_id` when
    /// retrying: resending it with the same message returns the same receipt.
    /// Fails with `steer_not_available` when no turn is running or can take
    /// one; send the message later as an ordinary [`Client::chat`] message.
    pub fn submit_input(
        &self,
        agent_id: impl Into<AgentId>,
        session_id: impl Into<SessionId>,
        request_id: impl Into<String>,
        message: UserMessage,
    ) -> Call<SubmitSessionInput> {
        let agent_id = agent_id.into();
        let session_id = session_id.into();
        Call::new(
            self.client,
            Op::SubmitSessionInput {
                agent_id: agent_id.as_str(),
                session_id: session_id.as_str(),
            },
            SubmitSessionInput {
                request_id: request_id.into(),
                message,
            },
        )
    }

    /// Lists a session's steer receipts in arrival order, with the session's
    /// current [activity](InputsPage::activity).
    pub fn inputs(
        &self,
        agent_id: impl Into<AgentId>,
        session_id: impl Into<SessionId>,
    ) -> Call<ListSessionInputs> {
        let agent_id = agent_id.into();
        let session_id = session_id.into();
        Call::new(
            self.client,
            Op::ListSessionInputs {
                agent_id: agent_id.as_str(),
                session_id: session_id.as_str(),
            },
            ListSessionInputs::default(),
        )
    }

    /// Records a stop for a running turn and returns at once; the turn's own
    /// stream keeps running until it ends. Retrying with the same `turn_id`
    /// never stops a later turn.
    pub fn stop(
        &self,
        agent_id: impl Into<AgentId>,
        session_id: impl Into<SessionId>,
        turn_id: impl Into<TurnId>,
    ) -> Call<StopSessionTurn> {
        let agent_id = agent_id.into();
        let session_id = session_id.into();
        Call::new(
            self.client,
            Op::StopSessionTurn {
                agent_id: agent_id.as_str(),
                session_id: session_id.as_str(),
            },
            StopSessionTurn {
                turn_id: turn_id.into(),
            },
        )
    }
}

/// Request state of [`Sessions::list`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSessions {
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<Cursor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u8>,
}
impl_request!(ListSessions => Page<Session>; encode = query; decode = json; paginate);

impl Call<ListSessions> {
    /// Only this end user's sessions. An empty string selects sessions
    /// without an end user.
    pub fn user_id(self, user_id: impl Into<String>) -> Self {
        self.map(|request| request.user_id = Some(user_id.into()))
    }

    /// Sessions per page, 1 to 200. Defaults to 50.
    pub fn limit(self, limit: u8) -> Self {
        self.map(|request| request.limit = Some(limit))
    }
}

/// Request state of [`Sessions::list_latest`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListLatestSessions {
    #[serde(skip_serializing_if = "Option::is_none")]
    user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    by_agent: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<Cursor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u8>,
}
impl_request!(ListLatestSessions => Page<LatestSession>; encode = query; decode = json; paginate);

impl Call<ListLatestSessions> {
    /// Only this end user's sessions. An empty string selects sessions
    /// without an end user.
    pub fn user_id(self, user_id: impl Into<String>) -> Self {
        self.map(|request| request.user_id = Some(user_id.into()))
    }

    /// `true` returns at most one session per agent: its latest. Defaults to
    /// `false`.
    pub fn by_agent(self, by_agent: bool) -> Self {
        self.map(|request| request.by_agent = Some(by_agent))
    }

    /// Sessions per page, 1 to 200. Defaults to 50.
    pub fn limit(self, limit: u8) -> Self {
        self.map(|request| request.limit = Some(limit))
    }
}

/// Request state of [`Sessions::get`].
#[derive(Clone, Debug)]
pub struct GetSession;
impl_request!(GetSession => SessionDetail; encode = none; decode = json);

/// Request state of [`Sessions::fork`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForkSession {
    message_id: String,
    #[serde(skip)]
    idempotency_key: String,
}

impl ForkSession {
    fn into_payload(self) -> Result<Payload> {
        if self.idempotency_key.is_empty() {
            return Err(crate::Error::InvalidArgument(
                "idempotency key must not be empty".to_owned(),
            ));
        }
        let key = HeaderValue::from_str(&self.idempotency_key).map_err(|_| {
            crate::Error::InvalidArgument("idempotency key is not a valid header value".to_owned())
        })?;
        Payload::default()
            .header(http::HeaderName::from_static("idempotency-key"), key)
            .json(&self)
    }
}
impl_request!(ForkSession => SessionDetail; encode = (Self::into_payload); decode = json);

/// Request state of [`Sessions::messages`].
#[derive(Clone, Debug, Default)]
pub struct ListSessionMessages {
    position: Option<Position>,
    limit: Option<u8>,
}

/// Where in the transcript to start: `Before` pages backwards, `After` polls
/// forwards.
#[derive(Clone, Debug)]
enum Position {
    Before(Cursor),
    After(Cursor),
}

/// The wire query of [`ListSessionMessages`]: `cursor` and `after` are one
/// position, so only one is ever sent.
#[derive(Serialize)]
struct MessagesQuery<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<&'a Cursor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    after: Option<&'a Cursor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u8>,
}

impl ListSessionMessages {
    fn into_payload(self) -> Result<Payload> {
        let (cursor, after) = match &self.position {
            Some(Position::Before(cursor)) => (Some(cursor), None),
            Some(Position::After(cursor)) => (None, Some(cursor)),
            None => (None, None),
        };
        Payload::default().query(&MessagesQuery {
            cursor,
            after,
            limit: self.limit,
        })
    }
}
impl_request!(ListSessionMessages => MessagesPage; encode = (Self::into_payload); decode = json);

impl Paginate for ListSessionMessages {
    fn set_cursor(&mut self, cursor: Cursor) {
        // A forward poll stays forward so `.items()` keeps polling after
        // `.after(_)` set the direction.
        match self.position.as_mut() {
            Some(Position::After(current)) => *current = cursor,
            _ => self.position = Some(Position::Before(cursor)),
        }
    }
}

impl Call<ListSessionMessages> {
    /// Returns messages added after a saved
    /// [`latest_cursor`](MessagesPage::latest_cursor), to poll a live
    /// transcript. It replaces any [`.cursor()`](Call::cursor), and a later
    /// `.cursor(_)` keeps the forward direction, so
    /// [`.items()`](Call::items) keeps walking forward.
    pub fn after(self, cursor: Cursor) -> Self {
        self.map(|request| request.position = Some(Position::After(cursor)))
    }

    /// Messages per page, 1 to 200. Defaults to 50.
    pub fn limit(self, limit: u8) -> Self {
        self.map(|request| request.limit = Some(limit))
    }
}

/// Request state of [`Sessions::tool_approvals`].
#[derive(Clone, Debug)]
pub struct ListToolApprovals;
impl_request!(ListToolApprovals => ToolApprovalList; encode = none; decode = json);

/// Request state of [`Sessions::delete`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSession {
    delete_artifacts: bool,
}
impl_request!(DeleteSession => (); encode = query; decode = none);

/// Request state of [`Sessions::submit_input`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitSessionInput {
    request_id: String,
    message: UserMessage,
}
impl_request!(SubmitSessionInput => SubmittedInput; encode = json; decode = json);

/// Request state of [`Sessions::inputs`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSessionInputs {
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<Cursor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    include_completed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u8>,
}
impl_request!(ListSessionInputs => InputsPage; encode = query; decode = json; paginate);

impl Call<ListSessionInputs> {
    /// Also returns `committed` and `not_placed` receipts. Defaults to
    /// `false`.
    pub fn include_completed(self, include_completed: bool) -> Self {
        self.map(|request| request.include_completed = Some(include_completed))
    }

    /// Receipts per page, 1 to 200. Defaults to 100.
    pub fn limit(self, limit: u8) -> Self {
        self.map(|request| request.limit = Some(limit))
    }
}

/// Request state of [`Sessions::stop`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StopSessionTurn {
    turn_id: TurnId,
}
impl_request!(StopSessionTurn => StoppedTurn; encode = json; decode = json);

/// A session in an agent's session list. [`Sessions::get`] and
/// [`Sessions::fork`] return a [`SessionDetail`].
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct Session {
    /// The session's ID.
    pub id: SessionId,
    /// The number of stored messages.
    pub message_count: u64,
    /// A short preview of the last message, or `None` when there is none.
    pub last_message_preview: Option<String>,
    /// The end user, or an empty string for a tenant-level session.
    pub user_id: String,
    /// Your own data.
    pub metadata: Metadata,
    /// When the session was created.
    pub created_at: Timestamp,
    /// When the session last changed.
    pub updated_at: Timestamp,
}

/// A session with its saved agent configuration, returned by
/// [`Sessions::get`] and [`Sessions::fork`].
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct SessionDetail {
    /// The session's ID.
    pub id: SessionId,
    /// The number of stored messages.
    pub message_count: u64,
    /// A short preview of the last message, or `None` when there is none.
    pub last_message_preview: Option<String>,
    /// The end user, or an empty string for a tenant-level session.
    pub user_id: String,
    /// Your own data.
    pub metadata: Metadata,
    /// When the session was created.
    pub created_at: Timestamp,
    /// When the session last changed.
    pub updated_at: Timestamp,
    /// The agent configuration saved for the session's first turn. Later
    /// agent edits do not change it.
    pub agent_config: crate::agents::AgentConfig,
    /// The source session and message of a fork, `None` for an ordinary
    /// session.
    pub forked_from: Option<ForkedFrom>,
}

/// The session and message a forked session started from.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ForkedFrom {
    /// The source session.
    pub session_id: SessionId,
    /// The last inherited message.
    pub message_id: String,
}

crate::open_enum! {
    /// A built-in tool.
    pub enum BuiltinTool {
        /// `read`
        Read = "read",
        /// `write`
        Write = "write",
        /// `edit`
        Edit = "edit",
        /// `grep`
        Grep = "grep",
        /// `glob`
        Glob = "glob",
        /// `bash`
        Bash = "bash",
        /// `publish_artifacts`
        PublishArtifacts = "publish_artifacts",
        /// `write_todos`
        WriteTodos = "write_todos",
        /// `save_memory`
        SaveMemory = "save_memory",
        /// `get_memory`
        GetMemory = "get_memory",
        /// `search_memories`
        SearchMemories = "search_memories",
        /// `update_memory`
        UpdateMemory = "update_memory",
        /// `delete_memory`
        DeleteMemory = "delete_memory",
        /// `web_search`
        WebSearch = "web_search",
        /// `activate_skill`
        ActivateSkill = "activate_skill",
    }
}

/// The tool a tool approval refers to.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ToolReference {
    /// A built-in tool.
    Builtin {
        /// The tool's name.
        name: BuiltinTool,
    },
    /// A tool of an MCP connection.
    Mcp {
        /// The MCP connection.
        #[serde(rename = "connectionId")]
        connection_id: McpConnectionId,
        /// The tool's name on that connection.
        name: String,
    },
    /// A function a backend supplied for one chat request.
    Function {
        /// The function's name.
        name: String,
    },
    /// A tool kind this SDK version does not know.
    #[serde(other)]
    Unrecognized,
}

crate::open_enum! {
    /// Whether an agent accepts new turns.
    pub enum AgentStatus {
        /// `active`
        Active = "active",
        /// `disabled`
        Disabled = "disabled",
    }
}

/// A session in the latest-sessions list.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct LatestSession {
    /// The session's ID.
    pub id: SessionId,
    /// The number of stored messages.
    pub message_count: u64,
    /// A short preview of the last message, or `None` when there is none.
    pub last_message_preview: Option<String>,
    /// The end user, or an empty string for a tenant-level session.
    pub user_id: String,
    /// Your own data.
    pub metadata: Metadata,
    /// When the session was created.
    pub created_at: Timestamp,
    /// When the session last changed.
    pub updated_at: Timestamp,
    /// The session's agent.
    pub agent_id: AgentId,
    /// The agent's current model, or `None` when unconfigured.
    pub model: Option<String>,
    /// The agent's current reasoning level, or `None` for the provider
    /// default.
    pub thinking_level: Option<String>,
    /// The agent's current status.
    pub status: AgentStatus,
}

/// A page of a session's transcript.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct MessagesPage {
    /// The messages on this page.
    pub data: Vec<UiMessage>,
    /// The cursor of the next page, or `None` on the last page.
    pub next_cursor: Option<Cursor>,
    /// Save it and pass it to [`.after()`](Call::after) to poll for messages
    /// added later. `None` means there is nothing new to anchor on: keep the
    /// cursor you saved before.
    pub latest_cursor: Option<Cursor>,
}

impl Paginated for MessagesPage {
    type Item = UiMessage;

    fn next_cursor(&self) -> Option<&Cursor> {
        self.next_cursor.as_ref()
    }

    fn into_items(self) -> Vec<UiMessage> {
        self.data
    }
}

/// A session's pending tool approvals.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ToolApprovalList {
    /// The tool calls of the open approval round.
    pub data: Vec<ToolApproval>,
    /// The session's unfinished continuation, or `None` when there is none.
    pub continuation: Option<Continuation>,
}

/// One tool call waiting for a decision, or decided with its round.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ToolApproval {
    /// The tool the call targets, if known.
    #[serde(default)]
    pub tool: Option<ToolReference>,
    /// The message that proposed the call, if known.
    #[serde(default)]
    pub assistant_message_id: Option<String>,
    /// When the call was proposed, if known.
    #[serde(default)]
    pub created_at: Option<Timestamp>,
    /// When the call was decided, if it was.
    #[serde(default)]
    pub decided_at: Option<Timestamp>,
    /// Pass it in a decision to [`Client::continue_chat`].
    pub approval_id: ApprovalId,
    /// The decision so far.
    pub decision: ToolApprovalDecision,
    /// The exact arguments the agent proposed.
    pub input: serde_json::Map<String, serde_json::Value>,
    /// The reason given with the decision, if any.
    pub reason: Option<String>,
    /// The model's tool call ID.
    pub tool_call_id: String,
    /// The tool's name.
    pub tool_name: String,
}

crate::open_enum! {
    /// The decision of one tool approval.
    pub enum ToolApprovalDecision {
        /// `pending` — still waiting.
        Pending = "pending",
        /// `approved`
        Approved = "approved",
        /// `denied`
        Denied = "denied",
    }
}

/// The session's unfinished tool approval continuation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct Continuation {
    /// The continuation's ID.
    pub id: String,
    /// Where it stands.
    pub state: ContinuationState,
}

crate::open_enum! {
    /// Where a tool approval continuation stands.
    pub enum ContinuationState {
        /// `waiting`
        Waiting = "waiting",
        /// `running`
        Running = "running",
        /// `succeeded`
        Succeeded = "succeeded",
        /// `failed`
        Failed = "failed",
    }
}

/// What a session is doing right now.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct SessionActivity {
    /// The session's state.
    pub state: ActivityState,
    /// The running turn, when there is one.
    pub turn_id: Option<TurnId>,
}

crate::open_enum! {
    /// What a session is doing right now.
    pub enum ActivityState {
        /// `idle`
        Idle = "idle",
        /// `running`
        Running = "running",
        /// `stopping`
        Stopping = "stopping",
        /// `approval` — waiting for tool approval decisions.
        Approval = "approval",
    }
}

/// An input queued for a session.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct SessionInput {
    /// Your ID for the steer.
    pub request_id: String,
    /// Its place in the arrival order; never changes.
    pub sequence: u64,
    /// The message you sent.
    pub message: UserMessage,
    /// The receipt's state.
    pub state: InputState,
    /// The turn the steer was bound to.
    pub turn_id: TurnId,
    /// When the input was received.
    pub created_at: Timestamp,
    /// When the receipt last changed.
    pub updated_at: Timestamp,
    /// Why the input ended, when it did not commit.
    pub reason: Option<InputReason>,
}

crate::open_enum! {
    /// The state of a steer receipt.
    pub enum InputState {
        /// `accepted` — queued for the turn.
        Accepted = "accepted",
        /// `delivered` — handed to the turn.
        Delivered = "delivered",
        /// `committed` — saved in the session's history.
        Committed = "committed",
        /// `not_placed` — never reached the agent; safe to resend as an
        /// ordinary [`Client::chat`] message.
        NotPlaced = "not_placed",
        /// `uncertain` — the agent may have read it; never resend it
        /// automatically.
        Uncertain = "uncertain",
    }
}

crate::open_enum! {
    /// Why a steered input ended without committing.
    pub enum InputReason {
        /// `stopped`
        Stopped = "stopped",
        /// `failed`
        Failed = "failed",
        /// `owner_lost`
        OwnerLost = "owner_lost",
        /// `turn_finished`
        TurnFinished = "turn_finished",
    }
}

/// The outcome of queuing an input.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct SubmittedInput {
    /// The steer receipt.
    #[serde(rename = "data")]
    pub input: SessionInput,
    /// The session's current activity.
    pub activity: SessionActivity,
}

/// A page of a session's queued inputs.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct InputsPage {
    /// The inputs on this page.
    pub data: Vec<SessionInput>,
    /// The cursor of the next page, or `None` on the last page.
    pub next_cursor: Option<Cursor>,
    /// The session's current activity.
    pub activity: SessionActivity,
}

impl Paginated for InputsPage {
    type Item = SessionInput;

    fn next_cursor(&self) -> Option<&Cursor> {
        self.next_cursor.as_ref()
    }

    fn into_items(self) -> Vec<SessionInput> {
        self.data
    }
}

/// The outcome of stopping a turn.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct StoppedTurn {
    /// The stopped turn's ID.
    pub stopped_turn_id: TurnId,
    /// The session's current activity.
    pub activity: SessionActivity,
}

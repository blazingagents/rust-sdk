//! Sessions, their transcripts, inputs and tool approvals. Start from
//! [`Client::sessions`]. Turns are sent with [`Client::chat`].

use http::HeaderValue;
use serde::{Deserialize, Serialize};

use crate::call::{Call, Payload, impl_request};
use crate::client::{Client, Scope};
use crate::error::Result;
use crate::ids::{AgentId, SessionId, TurnId};
use crate::messages::UserMessage;
use crate::op::Op;
use crate::pagination::{Cursor, Page, Paginated};
use crate::transcript::UiMessage;

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
    /// Lists an agent's sessions, newest first.
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

    /// Lists the latest sessions across agents.
    pub fn list_latest(&self) -> Call<ListLatestSessions> {
        Call::new(
            self.client,
            Op::ListLatestSessions,
            ListLatestSessions::default(),
        )
    }

    /// Gets one session.
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

    /// Forks a session at `message_id`. Reuse `idempotency_key` when retrying.
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

    /// Lists a session's messages.
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

    /// Lists a session's pending tool approvals.
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

    /// Deletes a session and, with `delete_artifacts`, its artifacts.
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

    /// Queues a message for the running turn. Reuse `request_id` when retrying.
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

    /// Lists the inputs queued for a session.
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

    /// Stops a running turn.
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
}
impl_request!(ListSessions => Page<Session>; encode = query; decode = json; paginate);

/// Request state of [`Sessions::list_latest`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListLatestSessions {
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<Cursor>,
}
impl_request!(ListLatestSessions => Page<LatestSession>; encode = query; decode = json; paginate);

/// Request state of [`Sessions::get`].
#[derive(Clone, Debug)]
pub struct GetSession;
impl_request!(GetSession => Session; encode = none; decode = json);

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
        let key = HeaderValue::from_str(&self.idempotency_key).map_err(|_| {
            crate::Error::InvalidArgument("idempotency key is not a valid header value".to_owned())
        })?;
        Payload::default()
            .header(http::HeaderName::from_static("idempotency-key"), key)
            .json(&self)
    }
}
impl_request!(ForkSession => Session; encode = (Self::into_payload); decode = json);

/// Request state of [`Sessions::messages`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSessionMessages {
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<Cursor>,
}
impl_request!(ListSessionMessages => MessagesPage; encode = query; decode = json; paginate);

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
}
impl_request!(ListSessionInputs => InputsPage; encode = query; decode = json; paginate);

/// Request state of [`Sessions::stop`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StopSessionTurn {
    turn_id: TurnId,
}
impl_request!(StopSessionTurn => StoppedTurn; encode = json; decode = json);

/// A session.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct Session {}

/// A session in the latest-sessions list.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct LatestSession {}

/// A page of a session's transcript.
#[derive(Clone, Debug, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct MessagesPage {
    /// The messages on this page.
    pub data: Vec<UiMessage>,
    /// The cursor of the next page, or `None` on the last page.
    pub next_cursor: Option<Cursor>,
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
pub struct ToolApprovalList {}

/// The outcome of queuing an input.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct SubmittedInput {}

/// An input queued for a session.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct SessionInput {}

/// A page of a session's queued inputs.
#[derive(Clone, Debug, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct InputsPage {
    /// The inputs on this page.
    pub data: Vec<SessionInput>,
    /// The cursor of the next page, or `None` on the last page.
    pub next_cursor: Option<Cursor>,
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
pub struct StoppedTurn {}

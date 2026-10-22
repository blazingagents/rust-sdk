//! Relays a browser's `useChat` requests to an agent: a demo sign-in check,
//! one `POST /api/chat` endpoint that accepts the default `DefaultChatTransport`
//! body, and an SSE response the AI SDK reads.
//!
//! The browser needs nothing extra: wire `useChat` with
//! `DefaultChatTransport({ api: "/api/chat", headers: { Authorization: "Bearer <token>" } })`
//! and the relay maps its chat `id` to a session. Tool approval rounds are not
//! handled here; use [`Client::continue_chat`](blazing_agents::Client::continue_chat).
//!
//! Each End-user chats with their own Agent. The relay acts through a client
//! scoped with [`Client::for_user`], finds that user's Agent, and creates one
//! the first time. The API creates the Agent's Workspace for the same user. A
//! scoped turn needs both the Agent and its Workspace owned by the scoped
//! user, so an Agent made without the scope answers 404. Creating Agents needs
//! a Full API key.
//!
//! The response copies `Location`, `X-BA-Turn-Id` and `x-request-id` from the
//! API response, so the browser can read the Session ID, stop the Turn, and
//! quote the request ID to support.
//!
//! ```sh
//! BLAZING_AGENTS_API_KEY=... BLAZING_AGENTS_PROVIDER_ID=prv_... \
//!   BLAZING_AGENTS_MODEL=openai/gpt-6-luna DEMO_USER_TOKEN=demo-token \
//!   cargo run --example chat_relay
//! ```

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use blazing_agents::chat::ChatStream;
use blazing_agents::ids::{AgentId, ProviderId, SessionId};
use blazing_agents::messages::UserMessage;
use blazing_agents::{Client, Error, UserClient};
use serde::Deserialize;

/// The body `DefaultChatTransport` POSTs: the chat id and the whole history.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatRequest {
    id: String,
    messages: Vec<serde_json::Value>,
    #[serde(default = "submit_message")]
    trigger: String,
    #[serde(default)]
    message_id: Option<String>,
}

fn submit_message() -> String {
    "submit-message".to_owned()
}

struct AppState {
    client: Client,
    provider_id: ProviderId,
    model: String,
    demo_token: String,
    /// Each user's Agent. A real app stores the Agent ID with the user.
    agents: Mutex<HashMap<String, AgentId>>,
    /// useChat's chat `id` to the session it continues, per user. A real app
    /// persists this in its database.
    sessions: Mutex<HashMap<(String, String), SessionId>>,
}

enum RelayError {
    Unauthorized,
    BadRequest(&'static str),
    Agent(blazing_agents::Error),
}

impl RelayError {
    fn json(status: StatusCode, message: &'static str) -> Response {
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

impl IntoResponse for RelayError {
    fn into_response(self) -> Response {
        match self {
            Self::Unauthorized => Self::json(StatusCode::UNAUTHORIZED, "unauthorized"),
            Self::BadRequest(message) => Self::json(StatusCode::BAD_REQUEST, message),
            Self::Agent(error) => {
                eprintln!("agent call failed: {error}");
                Self::json(StatusCode::BAD_GATEWAY, "the agent call failed")
            }
        }
    }
}

/// The last user message, which is the one `useChat` just submitted.
fn latest_user_message(body: &ChatRequest) -> Result<UserMessage, RelayError> {
    body.messages
        .iter()
        .rev()
        .find(|message| message.get("role") == Some(&serde_json::json!("user")))
        .cloned()
        .ok_or(RelayError::BadRequest("no user message in the history"))
        .and_then(|message| {
            serde_json::from_value::<UserMessage>(message)
                .map_err(|_| RelayError::BadRequest("the last user message is invalid"))
        })
}

/// The End-user's own Agent: the newest one the scoped client lists, or a new
/// one. The scope makes the API own the Agent and its Workspace by this user.
async fn user_agent(state: &AppState, user: &UserClient, user_id: &str) -> Result<AgentId, Error> {
    if let Some(agent_id) = state.agents.lock().unwrap().get(user_id) {
        return Ok(agent_id.clone());
    }
    let agent = match user.agents().list().limit(1).await?.data.into_iter().next() {
        Some(agent) => agent,
        None => {
            user.agents()
                .create("Assistant")
                .model(state.provider_id.clone(), state.model.clone())
                .await?
        }
    };
    state
        .agents
        .lock()
        .unwrap()
        .insert(user_id.to_owned(), agent.id.clone());
    Ok(agent.id)
}

/// The headers `useChat` needs, plus the API response's `Location` (a new
/// Session's URL), `X-BA-Turn-Id` and `x-request-id`.
fn relay_headers(stream: &ChatStream) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    headers.insert(
        HeaderName::from_static("x-vercel-ai-ui-message-stream"),
        HeaderValue::from_static("v1"),
    );
    for name in [
        header::LOCATION,
        HeaderName::from_static("x-ba-turn-id"),
        HeaderName::from_static("x-request-id"),
    ] {
        if let Some(value) = stream.meta().headers().get(&name) {
            headers.insert(name, value.clone());
        }
    }
    headers
}

async fn chat(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<ChatRequest>,
) -> Result<Response, RelayError> {
    // Demo sign-in check: the bearer token maps to one user. Replace with your
    // own sign-in check.
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    if token != Some(state.demo_token.as_str()) {
        return Err(RelayError::Unauthorized);
    }
    let user_id = "user-1";

    let message = latest_user_message(&body)?;
    let user = state
        .client
        .for_user(user_id)
        .map_err(|_| RelayError::BadRequest("invalid user ID"))?;
    let agent_id = user_agent(&state, &user, user_id)
        .await
        .map_err(RelayError::Agent)?;
    let key = (user_id.to_owned(), body.id.clone());
    let stored = state.sessions.lock().unwrap().get(&key).cloned();
    let stream = match (body.trigger.as_str(), stored) {
        ("submit-message", Some(session_id)) => user
            .chat(&agent_id, message)
            .session(session_id)
            .await
            .map_err(RelayError::Agent)?,
        ("submit-message", None) => user
            .chat(&agent_id, message)
            .await
            .map_err(RelayError::Agent)?,
        ("regenerate-message", Some(session_id)) => {
            let call = user.regenerate(&agent_id, session_id, message);
            match body.message_id {
                Some(message_id) => call.message_id(message_id),
                None => call,
            }
            .await
            .map_err(RelayError::Agent)?
        }
        ("regenerate-message", None) => {
            return Err(RelayError::BadRequest("unknown chat id"));
        }
        _ => return Err(RelayError::BadRequest("unknown trigger")),
    };

    if body.trigger == "submit-message" {
        state
            .sessions
            .lock()
            .unwrap()
            .insert(key, stream.session_id().clone());
    }

    let headers = relay_headers(&stream);
    Ok((headers, Body::from_stream(stream)).into_response())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = Arc::new(AppState {
        client: Client::from_env()?,
        provider_id: ProviderId::from(std::env::var("BLAZING_AGENTS_PROVIDER_ID")?),
        model: std::env::var("BLAZING_AGENTS_MODEL")?,
        demo_token: std::env::var("DEMO_USER_TOKEN")?,
        agents: Mutex::new(HashMap::new()),
        sessions: Mutex::new(HashMap::new()),
    });
    let app = Router::new()
        .route("/api/chat", post(chat))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8787").await?;
    println!("listening on http://{}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}

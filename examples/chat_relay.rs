//! Relays a browser's chat requests to an agent: a demo sign-in check, one
//! `POST /api/chat` endpoint, and an SSE response the AI SDK `useChat` hook
//! reads.
//!
//! ```sh
//! BLAZING_AGENTS_API_KEY=... BLAZING_AGENTS_AGENT_ID=ag_... DEMO_USER_TOKEN=demo-token \
//!   cargo run --example chat_relay
//! ```

use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use blazing_agents::Client;
use blazing_agents::messages::UserMessage;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatRequest {
    message: UserMessage,
    #[serde(default)]
    session_id: Option<String>,
}

struct AppState {
    client: Client,
    agent_id: String,
    demo_token: String,
}

enum RelayError {
    Unauthorized,
    InvalidUserId,
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
            Self::InvalidUserId => Self::json(StatusCode::BAD_REQUEST, "invalid user ID"),
            Self::Agent(error) => {
                eprintln!("agent call failed: {error}");
                Self::json(StatusCode::BAD_GATEWAY, "the agent call failed")
            }
        }
    }
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

    let user = state
        .client
        .for_user(user_id)
        .map_err(|_| RelayError::InvalidUserId)?;
    let call = user.chat(&state.agent_id, body.message);
    let call = match body.session_id {
        Some(session_id) => call.session(session_id),
        None => call,
    };
    let stream = call.await.map_err(RelayError::Agent)?;

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
    headers.insert(
        HeaderName::from_static("x-session-id"),
        HeaderValue::from_str(stream.session_id().as_str())
            .expect("a session ID is printable ASCII"),
    );
    Ok((headers, Body::from_stream(stream)).into_response())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = Arc::new(AppState {
        client: Client::from_env()?,
        agent_id: std::env::var("BLAZING_AGENTS_AGENT_ID")?,
        demo_token: std::env::var("DEMO_USER_TOKEN")?,
    });
    let app = Router::new()
        .route("/api/chat", post(chat))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8787").await?;
    println!("listening on http://{}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}

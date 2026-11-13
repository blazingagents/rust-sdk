//! The backend of `useBlazingAgentsChat`: a demo sign-in check and one
//! `POST /api/chat` endpoint that answers the browser's `chat`, `history` and
//! `steer` requests, Tool approvals included.
//!
//! The browser needs nothing extra: wire
//! `useBlazingAgentsChat({ api: "/api/chat", headers: { Authorization: "Bearer <token>" } })`.
//! The browser keeps the Session ID from the `Location` header of the first
//! reply, so this server stores nothing per chat.
//!
//! Each End-user chats with their own Agent. [`ChatRelay`] acts through
//! `Client::for_user`, so the API reaches only that user's Agents and Sessions.
//! The sign-in callback finds the user's Agent and creates one the first time.
//! The API creates the Agent's Workspace for the same user. A scoped turn needs
//! both the Agent and its Workspace owned by the scoped user, so an Agent made
//! without the scope answers 404. Creating Agents needs a Full API key.
//!
//! ```sh
//! BLAZING_AGENTS_API_KEY=... BLAZING_AGENTS_PROVIDER_ID=prv_... \
//!   BLAZING_AGENTS_MODEL=openai/gpt-6-luna DEMO_USER_TOKEN=demo-token \
//!   cargo run --example chat_relay
//! ```

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{Request, State};
use axum::http::header;
use axum::response::Response;
use axum::routing::post;
use blazing_agents::ids::{AgentId, ProviderId};
use blazing_agents::relay::{ChatRelay, RelayBody, RelayContext};
use blazing_agents::{Client, Error};

struct Demo {
    client: Client,
    provider_id: ProviderId,
    model: String,
    token: String,
    /// Each user's Agent. A real app stores the Agent ID with the user.
    agents: Mutex<HashMap<String, AgentId>>,
}

impl Demo {
    /// The End-user's own Agent: the newest one the scoped client lists, or a
    /// new one.
    async fn agent_for(&self, user_id: &str) -> Result<AgentId, Error> {
        if let Some(agent_id) = self.agents.lock().unwrap().get(user_id) {
            return Ok(agent_id.clone());
        }
        let user = self.client.for_user(user_id)?;
        let agent = match user.agents().list().limit(1).await?.data.into_iter().next() {
            Some(agent) => agent,
            None => {
                user.agents()
                    .create("Assistant")
                    .model(self.provider_id.clone(), self.model.clone())
                    .await?
            }
        };
        self.agents
            .lock()
            .unwrap()
            .insert(user_id.to_owned(), agent.id.clone());
        Ok(agent.id)
    }
}

async fn chat(State(relay): State<ChatRelay>, request: Request) -> Response<RelayBody> {
    relay.handle(request).await
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let demo = Arc::new(Demo {
        client: Client::from_env()?,
        provider_id: ProviderId::from(std::env::var("BLAZING_AGENTS_PROVIDER_ID")?),
        model: std::env::var("BLAZING_AGENTS_MODEL")?,
        token: std::env::var("DEMO_USER_TOKEN")?,
        agents: Mutex::new(HashMap::new()),
    });
    let relay = ChatRelay::new(demo.client.clone(), move |request| {
        let demo = demo.clone();
        async move {
            // Demo sign-in check: the bearer token maps to one user. Replace
            // it with your own. `None` answers 401; an error answers 500.
            let token = request
                .headers()
                .get(header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.strip_prefix("Bearer "));
            if token != Some(demo.token.as_str()) {
                return Ok(None);
            }
            let user_id = "user-1";
            let agent_id = demo.agent_for(user_id).await?;
            Ok(Some(RelayContext::new(agent_id, user_id)))
        }
    });
    let app = Router::new()
        .route("/api/chat", post(chat))
        .with_state(relay);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8787").await?;
    println!("listening on http://{}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}

<div align="center">
  <a href="https://docs.blazingagents.com">
    <img src="https://raw.githubusercontent.com/blazingagents/docs/main/public/brand/icon.svg" alt="Blazing Agents logo" width="96">
  </a>
  <h1>Blazing Agents Rust SDK</h1>
  <p>Build production agents in Rust with a typed client for the Blazing Agents API.</p>
  <p>
    <a href="https://docs.blazingagents.com/sdk/rust">Documentation</a>
  </p>
</div>

The official Rust SDK is an asynchronous client for the Blazing Agents `/v1`
API that your backend uses to manage Agents and run them for your users. It
runs on Tokio and supports Rust 1.85 and newer.

## Features

- A typed method for every `/v1` operation an API key can call: Agents,
  Skills, Sessions, Memories, Prompts, Tasks, Workspaces, Artifacts,
  Providers, MCP Connections, Chat Connections, usage, spending limits, Tenant
  settings, and merchant monetization.
- Chat streams in the AI SDK UI message stream format, with steering, Tool
  approvals, and backend functions that run in your process.
- Text and structured-object generation.
- End-user scoped clients through `for_user`.
- Lazy pagination, uploads, and streamed downloads.
- Request IDs and typed errors.
- JSON Schemas for structured output derived from your Rust types with
  `schemars`.

## Installation

```sh
cargo add blazing-agents
cargo add tokio --features macros,rt-multi-thread
```

TLS uses rustls by default. To use native TLS instead, add the crate with
`cargo add blazing-agents --no-default-features --features native-tls,schemars`.

## Quick start

Create a Full API key in the Blazing Agents dashboard and set
`BLAZING_AGENTS_API_KEY` in your server environment. The key acts for your
whole Tenant, so keep it on your backend and never send it to a browser.

```rust
use blazing_agents::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::from_env()?;

    let answer = client
        .completion("ag_...", "Write a friendly welcome message.")
        .await?;
    println!("{answer}");
    Ok(())
}
```

Replace `ag_...` with an Agent ID from the dashboard.

For requests from your users, scope the client to the user ID that your
backend verified at sign-in, never to an ID from the request body. A scoped
client reaches only that End-user's Agents, Sessions, and Workspace. An
unscoped Agent shares one Workspace across all your users, so give each user
an Agent of their own:

```rust
use blazing_agents::chat::ChatStream;
use blazing_agents::ids::AgentId;
use blazing_agents::messages::UserMessage;

async fn create_agent_for_user(client: &Client, user_id: &str) -> blazing_agents::Result<AgentId> {
    let agent = client
        .for_user(user_id)?
        .agents()
        .create("Assistant")
        .model("prv_...", "openai/gpt-6-luna")
        .await?;
    Ok(agent.id)
}

async fn chat_as_user(
    client: &Client,
    user_id: &str,
    agent_id: &AgentId,
    text: &str,
) -> blazing_agents::Result<ChatStream> {
    client
        .for_user(user_id)?
        .chat(agent_id, UserMessage::text(text))
        .await
}
```

Call `create_agent_for_user` once per user and store the Agent ID with the
user. A scoped chat with an Agent created without the scope fails with `404`.
`chat_as_user` returns a `ChatStream`, a `Stream` of server-sent event bytes. Send it to the browser as a `text/event-stream` response, for example
with axum's `Body::from_stream`, and AI SDK `useChat` reads it.

## Examples

The [`examples/`](examples) directory has three programs. Each file's header
lists the environment variables it reads and the command that runs it.

- `quickstart` saves a Prompt, runs it as a completion, and chats with an
  Agent.
- `chat_relay` is an axum server that relays `useChat` requests from a
  signed-in user to that user's own Agent.
- `structured_output` generates an object whose JSON Schema is derived from
  a Rust type.

The [examples repository](https://github.com/blazingagents/examples) has
complete TypeScript and Python apps built on the Blazing Agents SDKs.

## Documentation

Read the [Rust SDK documentation](https://docs.blazingagents.com/sdk/rust)
for client setup, End-user scoping, chat and generation, a guide to each
resource, and error handling.

## Development

```console
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo cov
cargo test --workspace --doc
cargo xtask spec --check
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
cargo +1.85 test --workspace --locked
cargo test -p blazing-agents --no-default-features --features native-tls,schemars
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the crate's layout and rules.

## License

[MIT](LICENSE)

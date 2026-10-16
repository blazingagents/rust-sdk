<div align="center">
  <a href="https://docs.blazingagents.com">
    <img src="https://raw.githubusercontent.com/blazingagents/docs/main/public/brand/icon.svg" alt="Blazing Agents logo" width="96">
  </a>
  <h1>Blazing Agents Rust SDK</h1>
  <p>Build production agents in Rust with a typed client for the Blazing Agents API.</p>
  <p>
    <a href="https://docs.blazingagents.com">Documentation</a>
  </p>
</div>

The official Rust SDK for the Blazing Agents `/v1` API. It is in development
and not yet published to crates.io: the client, transport and prompts are
complete, and the other resources are being filled in.

## Installation

Until the first release, depend on the repository:

```toml
[dependencies]
blazing-agents = { git = "https://github.com/blazingagents/rust-sdk" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
futures-util = "0.3"
```

The crate is async and runs on Tokio. TLS uses rustls by default; enable the
`native-tls` feature instead with `default-features = false, features =
["native-tls", "schemars"]`.

## Quick start

Set `BLAZING_AGENTS_API_KEY` to a tenant API key, then:

```rust,no_run
use blazing_agents::Client;
use blazing_agents::generation::SavedPrompt;
use blazing_agents::messages::UserMessage;
use futures_util::TryStreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::from_env()?;
    let agent_id = "ag_4kP9sT2vXq7LmN3a";

    let prompt = client
        .prompts()
        .create("Greeting", "Greet {{name}} in one sentence.")
        .await?;

    let answer = client
        .completion(agent_id, SavedPrompt::new(&prompt.id).var("name", "Ada"))
        .await?;
    println!("{answer}");

    let mut stream = client.chat(agent_id, UserMessage::text("Hello!")).await?;
    println!("session {}", stream.session_id());
    while let Some(event) = stream.try_next().await? {
        print!("{}", String::from_utf8_lossy(&event));
    }
    Ok(())
}
```

The same program is in [`examples/quickstart.rs`](examples/quickstart.rs);
run it with `BLAZING_AGENTS_AGENT_ID=ag_... cargo run --example quickstart`.

Every method returns a `Call` that `.await` sends. Required inputs are
arguments; optional inputs are setters, such as
`client.prompts().list().agent_id(id).limit(25)`. Lists also offer `.items()`
to walk every page lazily. `client.for_user(user_id)?` returns a client that
acts for one end user and omits tenant administration.

## License

[MIT](LICENSE)

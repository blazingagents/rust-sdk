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

The official Rust SDK provides an asynchronous client for the Blazing Agents
`/v1` API, built on Tokio. It supports Rust 1.85 and newer.

## Features

- Typed request builders and response models; unknown fields and enum values
  still decode.
- Agent, Workspace, Skill, Provider, MCP connection, Prompt, Memory, Session,
  Artifact, Task, Chat Connection and delivery, Merchant, Usage, and Tenant
  management.
- Chat streams of AI SDK UI message stream events, ready to relay to
  `useChat`, with message batches, tool approvals and backend functions.
- Text and structured-object generation, buffered or streamed, with
  schemars-derived schemas.
- `for_user` user-scoped clients without tenant administration methods.
- Lazy pagination, uploads and streamed downloads.
- Request correlation, per-call timeouts and headers, response observation,
  and typed errors with credential redaction.

## Installation

```sh
cargo add blazing-agents
cargo add tokio --features macros,rt-multi-thread
```

Or add it to `Cargo.toml`:

```toml
[dependencies]
blazing-agents = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

The crate is async and runs on Tokio. TLS uses rustls by default; enable the
`native-tls` feature instead with `default-features = false, features =
["native-tls", "schemars"]`.

## Quick start

Create a Tenant API key in the Blazing Agents dashboard and set
`BLAZING_AGENTS_API_KEY`, then:

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

Replace `ag_...` with an agent ID from the dashboard. See
[`examples/`](examples) — `quickstart`, `chat_relay` and `structured_output` —
and run any of them with
`BLAZING_AGENTS_API_KEY=... BLAZING_AGENTS_AGENT_ID=ag_... cargo run --example quickstart`.
`chat_relay` also needs `DEMO_USER_TOKEN`, the bearer token the demo accepts:
`BLAZING_AGENTS_API_KEY=... BLAZING_AGENTS_AGENT_ID=ag_... DEMO_USER_TOKEN=... cargo run --example chat_relay`.

## Documentation

Read the [Rust SDK documentation](https://docs.blazingagents.com/sdk/rust)
for authentication, resource guides, chat and generation, error handling, and
the complete API reference.

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

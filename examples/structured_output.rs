//! Generates a structured object whose JSON Schema is derived from a Rust type.
//!
//! ```sh
//! BLAZING_AGENTS_API_KEY=... BLAZING_AGENTS_AGENT_ID=ag_... \
//!   cargo run --example structured_output --features schemars
//! ```

use blazing_agents::Client;
use blazing_agents::generation::ObjectSchema;

/// The triage of an incoming support ticket.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct Triage {
    /// `low`, `medium` or `high`.
    priority: String,
    /// A one-line summary of the issue.
    summary: String,
    /// Labels for routing the ticket.
    tags: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::from_env()?;
    let agent_id = std::env::var("BLAZING_AGENTS_AGENT_ID")?;

    let triage = client
        .object(
            &agent_id,
            "Triage this ticket: \"The export button returns a blank CSV for every report since Tuesday.\"",
            ObjectSchema::<Triage>::derive(),
        )
        .await?;

    println!("priority: {}", triage.priority);
    println!("summary:  {}", triage.summary);
    println!("tags:     {}", triage.tags.join(", "));
    Ok(())
}

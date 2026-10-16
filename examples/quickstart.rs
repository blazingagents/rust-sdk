//! Saves a prompt, runs it as a completion, then chats with the agent.
//!
//! ```sh
//! BLAZING_AGENTS_API_KEY=... BLAZING_AGENTS_AGENT_ID=ag_... cargo run --example quickstart
//! ```

use blazing_agents::Client;
use blazing_agents::generation::SavedPrompt;
use blazing_agents::messages::UserMessage;
use futures_util::TryStreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::from_env()?;
    let agent_id = std::env::var("BLAZING_AGENTS_AGENT_ID")?;

    let prompt = client
        .prompts()
        .create("Greeting", "Greet {{name}} in one sentence.")
        .await?;
    println!("saved {} with variables {:?}", prompt.id, prompt.variables);

    let answer = client
        .completion(&agent_id, SavedPrompt::new(&prompt.id).var("name", "Ada"))
        .await?;
    println!("{answer}");

    let mut stream = client
        .chat(&agent_id, UserMessage::text("Hello! What can you do?"))
        .await?;
    println!("session {}", stream.session_id());
    while let Some(event) = stream.try_next().await? {
        print!("{}", String::from_utf8_lossy(&event));
    }

    client.prompts().delete(&prompt.id).await?;
    Ok(())
}

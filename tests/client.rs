mod support;

use blazing_agents::Client;
use blazing_agents::client::API_KEY_ENV;
use blazing_agents::error::ConfigError;
use support::requests;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

// The only test in this file mutates the environment, so nothing races it.
#[tokio::test]
async fn from_env_reads_the_api_key_from_the_environment() {
    let saved = std::env::var(API_KEY_ENV).ok();
    // SAFETY: this test is the only test in its binary, so no other thread
    // reads or writes the environment.
    unsafe { std::env::set_var(API_KEY_ENV, "env-key") };

    let client = Client::from_env().unwrap();
    assert!(!format!("{client:?}").contains("env-key"));

    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    Client::builder()
        .base_url(server.uri())
        .build()
        .unwrap()
        .prompts()
        .delete("prompt_1")
        .await
        .unwrap();
    Client::builder()
        .api_key("explicit")
        .base_url(server.uri())
        .build()
        .unwrap()
        .prompts()
        .delete("prompt_1")
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(sent[0].headers["authorization"], "Bearer env-key");
    assert_eq!(sent[1].headers["authorization"], "Bearer explicit");

    // SAFETY: same as above.
    unsafe { std::env::set_var(API_KEY_ENV, "") };
    assert!(matches!(
        Client::from_env(),
        Err(ConfigError::MissingApiKey)
    ));

    // SAFETY: same as above.
    unsafe { std::env::remove_var(API_KEY_ENV) };
    assert!(matches!(
        Client::from_env(),
        Err(ConfigError::MissingApiKey)
    ));

    if let Some(value) = saved {
        // SAFETY: same as above.
        unsafe { std::env::set_var(API_KEY_ENV, value) };
    }
}

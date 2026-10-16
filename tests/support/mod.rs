//! Shared wiremock harness. Every integration test file includes it with
//! `mod support;`, and each uses the helpers it needs.

#![allow(dead_code)]

use blazing_agents::Client;
use wiremock::{MockServer, Request, ResponseTemplate};

/// A mock server and a tenant client pointed at it.
pub async fn mock() -> (MockServer, Client) {
    let server = MockServer::start().await;
    let client = Client::builder()
        .api_key("test-key")
        .base_url(server.uri())
        .build()
        .expect("a mock server URL is a valid base URL");
    (server, client)
}

/// A JSON response.
pub fn json(status: u16, body: serde_json::Value) -> ResponseTemplate {
    ResponseTemplate::new(status).set_body_json(body)
}

/// An API error envelope.
pub fn api_error(status: u16, code: &str, message: &str) -> ResponseTemplate {
    json(
        status,
        serde_json::json!({ "error": { "code": code, "message": message } }),
    )
}

/// The received requests, in order.
pub async fn requests(server: &MockServer) -> Vec<Request> {
    server
        .received_requests()
        .await
        .expect("request recording is on")
}

/// The JSON body of a received request.
pub fn json_body(request: &Request) -> serde_json::Value {
    request.body_json().expect("the request body is JSON")
}

# Changelog

## Unreleased

### Added

- `AgentTool::WebFetch` (`web_fetch`) joins the Agent tool catalog (`AGENT_TOOLS`). It reads a public web page or text document by URL and returns markdown, and needs no Workspace. Like every tool it is off until you list it in `tools`, and an approval policy can name it. `web_fetch` is now a reserved function name. The `web` group description now reads "Search the public web and read pages by URL."

### Fixed

- A credential-bearing call now fails with `Error::CredentialEcho` when the response carries a `credential` property, such as a merchant connection returning its stored key. The SDK previously caught only a `credential` value it had just submitted.

### Removed

The platform has no production users yet, so both removals ship in one release without a deprecation step.

- Tenant quotas. The API no longer has a monthly token or request limit on Tenant settings, so `TenantSettings::quota`, `tenant::Quota`, `tenant::QuotaInput`, the `quota` and `clear_quota` setters of `client.tenant().update()` and `ErrorCode::QuotaExceeded` are gone. Delete the call and stop matching `quota_exceeded`. Model spending limits (`client.tenant().update_spending_limit`) stay:

  ```rust
  // Before
  client
      .tenant()
      .update()
      .quota(QuotaInput::new(1).monthly_token_limit(5_000_000))
      .await?;
  // After
  client.tenant().update().name("Acme").await?;
  ```

- The MCP connection `oauth_client_credentials` auth type. `NewMcpConnection::OAuthClientCredentials`, `McpReconnect::OAuthClientCredentials` and `McpAuthType::OAuthClientCredentials` are gone. Connect the server with `Bearer` and a token you issue, or with `OAuthAuthorizationCode`, which is saved as `needs_auth` until an admin signs in through the dashboard and keeps its optional `client` and `scope`:

  ```rust
  // Before
  NewMcpConnection::OAuthClientCredentials {
      name: "Docs".to_owned(),
      url: "https://mcp.example.com/mcp".to_owned(),
      client: OAuthClient::new("client-42", "secret"),
      scope: None,
  }
  // After
  NewMcpConnection::Bearer {
      name: "Docs".to_owned(),
      url: "https://mcp.example.com/mcp".to_owned(),
      bearer_token: token,
  }
  ```

## 0.5.0 (2026-10-22)

### Changed

- Memory docs say a general memory is seen by unscoped turns only; a turn made through `for_user` sees only its end user's memories.

### Removed

- `MerchantUsageEvent::workflow_issue` and `MerchantWorkflowIssue`. The API
  no longer sends `workflowIssue`, and the SDK ignores it if it appears. Read
  `next_action` instead. It is `MerchantNextAction::Retry` for a `pending`
  event whose automatic delivery has stopped, which is when `workflow_issue`
  was set:

  ```rust
  // Before
  if event.workflow_issue.is_some() {
      client.merchant_usage_events().retry(event.id.clone()).await?;
  }
  // After
  if event.next_action == MerchantNextAction::Retry {
      client.merchant_usage_events().retry(event.id.clone()).await?;
  }
  ```

## 0.4.1 (2026-10-22)

### Changed

- The vendored contract now matches the platform, which no longer sends the
  merchant connection `status`, the usage summary counts `uncertain` and
  `unmapped`, or the steer receipt states `delivered` and `uncertain`. The SDK
  stopped reading them in 0.4.0, so no code changes.

## 0.4.0 (2026-10-22)

### Removed

- Chat delivery listing and repair: `Client::chat_deliveries`,
  `ChatDeliveries`, `chat_connections().deliveries()`,
  `chat_connections().repair_delivery()`, `ChatDelivery`,
  `TenantChatDelivery`, `ChatDeliveryResult`, `ChatDeliveryKind`,
  `ChatDeliveryStatus`, `ChatDeliveryListStatus` and `ids::ChatDeliveryId`.
  The API no longer keeps a delivery outbox: the turn that answers a Slack or
  Telegram message posts the reply itself, and a reply that still fails stays
  in the session transcript. Delete calls to these methods; read the reply
  from the session with `sessions().messages()`.
- `merchant_usage_events().release()`, `MerchantUsageEventStatus::Unmapped`,
  `MerchantNextAction::BindAndRelease`, `MerchantUsageCounts::unmapped` and
  `ErrorCode::MerchantBindingRequired`. Usage from an end user with no
  customer binding is now recorded once as `discarded`, with
  `last_error_code` `MERCHANT_CUSTOMER_UNMAPPED`, and is never billed later.
  Bind the end user before their first turn, or enable the connection's guard
  so their turn is refused with `ErrorCode::MerchantCustomerUnmapped`.
- `MerchantConnectionStatus`, `MerchantConnection::status` and
  `merchant_connection().update().status()`. A connection has no
  `disconnected` state; to pause billing, replace
  `.update().status(MerchantConnectionStatus::Disconnected)` with
  `tenant().update().monetization_enabled(false)`.
- `MerchantUsageEventStatus::Uncertain` and `MerchantUsageCounts::uncertain`.
  An event whose delivery keeps failing ends `failed`, and
  `merchant_usage_events().retry()` is safe because providers drop
  duplicates by the event ID. Match `Failed` where you matched `Uncertain`.
- `InputState::Delivered` and `InputState::Uncertain`. A steer receipt is
  `accepted`, `committed` or `not_placed`. Treat `Delivered` as `Accepted`
  (wait), and `Uncertain` as `NotPlaced` (send the message again with
  `Client::chat`).
- `SpendingLimitPeriod::reserved_usd`. Spending is no longer reserved before
  a model request: the platform checks the spend before each request and
  adds the cost after it, and `available_usd` is the amount minus the spend.
  Delete reads of `reserved_usd`.
- `ErrorCode::CheckoutEvidenceMismatch`. The API never returns it; delete
  match arms or comparisons that name it.

### Added

- `ErrorCode::AgentBusy`. `agents().delete()` fails with it while the agent
  has a running turn or a queued or running task run.

### Changed

- `workspaces().delete()` returns once the workspace and its files are
  deleted (the API answers `204`, not `202`).
- `merchant_connection().retire()` fails with `merchant_connection_in_use`
  while a turn on the connection runs or one of its events is `pending`.

## 0.3.0 (2026-10-19)

- `TaskRun::error_code` is the `ErrorCode` a `blocked` or `failed` run ended
  with, such as `ErrorCode::UsageCreditRequired`, `ErrorCode::QuotaExceeded`
  or `ErrorCode::ProviderRequired`. It is `None` for every other run and for a
  failure without a code; `error` still carries the message.
  `tasks().get_run()` and `tasks().list_runs()` return it.

## 0.2.0 (2026-10-19)

- The built-in tool catalog moved from `agents` to the new `agent_tools`
  module, generated from the API contract: `BuiltinToolName`,
  `AgentToolGroupTag`, `AgentToolFileAccessTag`, `AgentToolGroup`,
  `AGENT_TOOL_GROUPS`, `AgentToolDefinition` and `AGENT_TOOLS`. Change
  `use blazing_agents::agents::{AGENT_TOOLS, BuiltinToolName}` to
  `use blazing_agents::agent_tools::{AGENT_TOOLS, BuiltinToolName}`.
- Every operation the API contract marks credential-bearing now fails with
  `Error::CredentialEcho` when its response carries a credential field or
  echoes a submitted secret. `chat_connections().deliveries()`,
  `chat_deliveries().list()` and `chat_connections().repair_delivery()` are
  newly checked.

## 0.1.0 (2026-10-17)

- Initial release.
- `agents::AGENT_TOOLS` and `agents::AGENT_TOOL_GROUPS` list the built-in
  tools with a description, `requires_workspace`, a `group`, and, on file
  operation tools, a `file_access` of `file_read`, `file_write` or `execute`.
  An agent whose file tools are all `file_read` cannot change workspace files.
- `Client` and `UserClient` scopes: `client.for_user(user_id)` sends the end
  user's ID on every request and has no tenant administration methods.
- One method per `/v1` operation across agents, workspaces, skills, providers,
  MCP connections, prompts, memories, sessions, artifacts, tasks, chat
  connections and deliveries, merchant, usage, and tenant. Every method
  returns a `Call` that `.await` sends, with per-call timeouts, headers and
  client request IDs.
- `chat`, `regenerate` and `continue_chat` stream AI SDK UI message stream
  events, ready to relay to `useChat`. `.functions(...)` attaches backend
  functions the agent calls during the turn. `continue_chat` takes an approval
  round's `chat::ToolApprovalDecision` values, and
  `sessions().tool_approvals()` reports each one's
  `sessions::ToolApprovalStatus`.
- `workspaces().delete()` returns a `WorkspaceDeletion`: `Completed` once the
  workspace is gone, or `Pending` while cleanup continues in the background.
- `UserPart` text and file parts keep fields the SDK does not model, such as
  `filename` and `providerMetadata`, and send them back unchanged, so relaying
  a browser message loses nothing.
- `completion`/`completion_stream` and `object`/`object_stream` generate text
  and structured objects, buffered or streamed; `ObjectSchema::derive()`
  builds the schema with schemars.
- `.pages()` and `.items()` walk list results lazily; uploads go through
  `Upload` and downloads stream through `Download`.
- Typed `Error` variants redact submitted credentials, and `on_response`
  observes every response.

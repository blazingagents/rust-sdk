# Changelog

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

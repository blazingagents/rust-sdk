# Changelog

## 0.1.0 (unreleased)

- Initial release.
- `Client` and `UserClient` scopes: `client.for_user(user_id)` sends the end
  user's ID on every request and has no tenant administration methods.
- One method per `/v1` operation across agents, workspaces, skills, providers,
  MCP connections, prompts, memories, sessions, artifacts, tasks, chat
  connections and deliveries, merchant, usage, and tenant. Every method
  returns a `Call` that `.await` sends, with per-call timeouts, headers and
  client request IDs.
- `chat`, `regenerate` and `continue_chat` stream AI SDK UI message stream
  events, ready to relay to `useChat`. `.functions(...)` attaches backend
  functions the agent calls during the turn.
- `completion`/`completion_stream` and `object`/`object_stream` generate text
  and structured objects, buffered or streamed; `ObjectSchema::derive()`
  builds the schema with schemars.
- `.pages()` and `.items()` walk list results lazily; uploads go through
  `Upload` and downloads stream through `Download`.
- Typed `Error` variants redact submitted credentials, and `on_response`
  observes every response.

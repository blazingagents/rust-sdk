# Contributing

## Commands

| Command | What it does |
| --- | --- |
| `cargo fmt --all --check` | Formatting |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Lints; warnings fail |
| `cargo test --workspace` | Unit, integration and doc tests |
| `cargo xtask spec --check` | Fails when `src/op.rs` or `src/error/code.rs` differ from what `spec/openapi.json` generates |
| `cargo xtask spec` | Regenerates those two files |
| `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` | Rustdoc; broken links fail |
| `cargo +1.85 test --workspace --locked` | The MSRV build |
| `cargo test -p blazing-agents --no-default-features --features native-tls,schemars` | The OpenSSL build |
| `cargo package -p blazing-agents --locked` | Builds the crate exactly as crates.io would receive it |

CI runs all of these plus a grep that fails on `allow(dead_code)` or
`expect(dead_code)` anywhere under `src/`.

To refresh the vendored spec, copy `../ba-platform/servers/api/openapi.json`
over `spec/openapi.json` and run `cargo xtask spec`.

## How the crate is laid out

- `src/op.rs` is generated: one `Op` variant per operation in the spec that
  has an `operationId`, under `#[deny(dead_code)]`. A method sends a request
  only by constructing a variant, so an operation without a method fails the
  build with `error: variant X is never constructed`. Never suppress that
  error; add the method.
- Each resource module (`src/agents.rs`, `src/prompts.rs`, ...) owns its
  accessor (`impl Client<..> { pub fn agents(&self) }`), its view struct, one
  method per operation, one request type per operation, its setters and its
  model types. Tenant-only resources are implemented on `Client<Tenant>` only,
  so a `UserClient` cannot name them.
- `src/call.rs` holds `Call<R>`, the sealed `Request` trait and the
  `impl_request!` macro. `src/http.rs` is the transport. `src/patch.rs` holds
  the private `Patch<T>` used for nullable PATCH and PUT fields.
- Generated and shared files (`Cargo.toml`, `Cargo.lock`, `src/lib.rs`,
  `src/call.rs`, `src/client.rs`, `src/http.rs`, `src/op.rs`) change only in
  foundation work, never in a resource worker's PR.

## Filling in a resource module

`src/prompts.rs` and `tests/prompts.rs` are the exemplar. Copy their shape.

1. Models. Give each `#[non_exhaustive]` model struct its fields from the
   spec's response schema: `camelCase` on the wire, `Option<T>` for nullable
   fields, `Timestamp` for date-times, typed IDs from `src/ids.rs`, `Metadata`
   for metadata objects, and `crate::open_enum!` for any value set the server
   may grow. Never add `deny_unknown_fields`.
2. Request inputs. Required inputs are already positional arguments. Add each
   optional input as a private field with
   `#[serde(skip_serializing_if = "Option::is_none")]` and a setter in
   `impl Call<X>`. A nullable field uses `Patch<T>` with
   `#[serde(skip_serializing_if = "Patch::is_keep")]`, a `x(value)` setter and
   a `clear_x()` setter. `anyOf` bodies are enums passed as the positional
   argument. Lists get their own `limit` setter, because maximums differ.
3. Credentials. A request that sends a secret passes
   `sensitive = (|request: &Self| vec![request.secret.clone()])` to
   `impl_request!`, so errors redact it. A response that must never carry a
   credential decodes with `decode = credential_safe_json`.
4. Pages whose shape is not `{ data, nextCursor }` implement `Paginated` in
   the module. A list that accepts `cursor` or `after` stores one position
   and keeps the direction in `set_cursor`.
5. Rustdoc on every public item, linking the matching page under
   <https://docs.blazingagents.com>.
6. Tests in `tests/<module>.rs` with the harness in `tests/support/mod.rs`.
   For each operation check the method, path, query and body (including an
   omitted field versus an explicit `null`), and decode a response with an
   unknown field. For each list, walk two pages with `.items()`. For a
   credential-bearing operation, check that an error body echoing the secret
   is redacted.

Done means every command in the table passes and
`grep -rnE "(allow|expect)\(dead_code\)" src/` prints nothing.

## Rules

- Never write `allow(dead_code)` or `expect(dead_code)` under `src/`. The
  dead-code lint is the coverage gate.
- Never hand-edit `src/op.rs` or `src/error/code.rs`.
- Comments explain a non-obvious why. Code says what.
- Breaking changes follow the two-step policy in `AGENTS.md`.

## Releasing

1. In a PR, bump `version` in `Cargo.toml` (run `cargo check` so `Cargo.lock`
   follows) and add a `## X.Y.Z` entry to `CHANGELOG.md`. A release that
   deprecates or removes something shows the migration, as `AGENTS.md`
   requires.
2. After it merges, tag the merge commit `vX.Y.Z` and publish a GitHub
   release from that tag. `.github/workflows/publish.yml` reruns CI, checks
   that the tag matches the `Cargo.toml` version, and publishes to crates.io
   with trusted publishing. Prereleases are not published.

One-time setup by the crate owner. crates.io can attach a trusted publisher
only to a crate that already exists, so the first version goes out by hand:

1. From a clean checkout of `main` at the release commit, run
   `CARGO_REGISTRY_TOKEN=<token> cargo publish -p blazing-agents --locked`
   with a crates.io token scoped to `blazing-agents` and `publish-new`, with
   the shortest expiry. Revoke the token right after.
2. In the crate's settings on crates.io, add a GitHub trusted publisher:
   owner `blazingagents`, repository `rust-sdk`, workflow `publish.yml`,
   environment `crates-io`.
3. Create the `crates-io` environment in the repository settings, optionally
   with required reviewers.
4. Publish the GitHub release for that first version as in step 2 above.
   The workflow sees the version already on crates.io and skips the upload.

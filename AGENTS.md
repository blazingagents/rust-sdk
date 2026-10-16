All changes must use a topic branch and merge through a PR after required checks pass. Never commit or push directly to `main`.

This crate is a typed client for the Blazing Agents `/v1` API. The public contract is `../ba-platform/servers/api/openapi.json`, vendored at `spec/openapi.json`; follow the TypeScript and Python SDKs (`../typescript-sdk`, `../python-sdk`) for resource coverage and naming.

README.md is the repository's brief for Tenants: what it is, features, installation, a quick start, and links to the documentation. Document feature behavior on the documentation site (`blazingagents/docs`), not in the README.

## Commands

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo cov` (tests under coverage; fails under 99% of lines or functions), `cargo test --workspace --doc`, `cargo xtask spec --check` and `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` are the gate; CI runs them plus the 1.85 MSRV build, the `native-tls` build and `cargo package -p blazing-agents --locked`. Run them before every commit.
- `cargo xtask spec` regenerates `src/op.rs` and `src/error/code.rs` from `spec/openapi.json`. Never edit those files by hand.
- `src/op.rs` is under `#[deny(dead_code)]`: an operation with no SDK method fails the build naming its variant. Never write `allow(dead_code)` or `expect(dead_code)` under `src/`; add the method instead.
- `CONTRIBUTING.md` holds the layout and the recipe for filling in a resource module; `src/prompts.rs` and `tests/prompts.rs` are the exemplar.

Breaking changes (a removed or renamed method, field, enum value, or endpoint, or a `!` commit) break Tenant apps already in production, so ship them in two steps. First release the new shape while the old one still works: the server and this SDK keep accepting the old input, and the old input is marked deprecated in its doc comment. Remove the old shape only in a later release. Every release that deprecates or removes something adds a `CHANGELOG.md` entry under its version that names what changed and shows the migration from the old call to the new one.

All changes must use a topic branch and merge through a PR after required checks pass. Never commit or push directly to `main`.

This crate is a typed client for the Blazing Agents `/v1` API. The public contract is `../ba-platform/servers/api/openapi.json`; follow the TypeScript and Python SDKs (`../typescript-sdk`, `../python-sdk`) for resource coverage and naming.

README.md is the repository's brief for Tenants: what it is, features, installation, a quick start, and links to the documentation. Document feature behavior on the documentation site (`blazingagents/docs`), not in the README.

Breaking changes (a removed or renamed method, field, enum value, or endpoint, or a `!` commit) break Tenant apps already in production, so ship them in two steps. First release the new shape while the old one still works: the server and this SDK keep accepting the old input, and the old input is marked deprecated in its doc comment. Remove the old shape only in a later release. Every release that deprecates or removes something adds a `CHANGELOG.md` entry under its version that names what changed and shows the migration from the old call to the new one.

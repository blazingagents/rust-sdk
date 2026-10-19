//! The official Rust SDK for the Blazing Agents `/v1` API.
//!
//! Build a [`Client`] from `BLAZING_AGENTS_API_KEY` with [`Client::from_env`],
//! or configure one with [`Client::builder`]. Resource accessors such as
//! [`Client::agents`] return one method per API operation; each returns a
//! [`Call`](call::Call) that is sent with `.await`. [`Client::for_user`] narrows
//! a client to one end user.
//!
//! Documentation: <https://docs.blazingagents.com>.

#![forbid(unsafe_code)]

pub mod agent_tools;
pub mod agents;
pub mod artifacts;
pub mod call;
pub mod chat;
pub mod chat_connections;
pub mod client;
pub mod error;
pub mod files;
pub mod functions;
pub mod generation;
pub mod ids;
pub mod mcp_connections;
pub mod memories;
pub mod merchant;
pub mod messages;
pub mod pagination;
pub mod prompts;
pub mod providers;
pub mod sessions;
pub mod skills;
pub mod spending_limits;
pub mod tasks;
pub mod tenant;
pub mod transcript;
pub mod types;
pub mod usage;
pub mod workspaces;

mod http;
mod op;
mod open_enum;
mod patch;

pub(crate) use open_enum::open_enum;

pub use client::{Client, UserClient};
pub use error::{Error, Result};

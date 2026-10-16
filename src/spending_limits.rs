//! Model spending limits, shared by agents and the tenant.

use serde::{Deserialize, Serialize};

/// A spending limit to set: a USD amount and a UTC reset schedule.
#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct SpendingLimitInput {}

/// A spending limit and its current period.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct SpendingLimitStatus {}

//! Model spending limits, shared by agents and the tenant.
//!
//! Documentation: <https://docs.blazingagents.com/platform/usage-and-quotas>.

use serde::{Deserialize, Serialize};

use crate::types::Timestamp;

crate::open_enum! {
    /// How often a spending limit resets.
    pub enum ResetInterval {
        /// `daily`
        Daily = "daily",
        /// `weekly`
        Weekly = "weekly",
        /// `biweekly`
        Biweekly = "biweekly",
        /// `monthly`
        Monthly = "monthly",
    }
}

/// A spending limit to set: a USD amount and a UTC reset schedule.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpendingLimitInput {
    amount_usd: f64,
    reset_start_date: String,
    reset_interval: ResetInterval,
}

impl SpendingLimitInput {
    /// A limit of `amount_usd` USD, more than 0 and at most 1,000,000,000, that
    /// resets every `reset_interval` starting on the UTC `YYYY-MM-DD` day
    /// `reset_start_date`.
    pub fn new(
        amount_usd: f64,
        reset_start_date: impl Into<String>,
        reset_interval: ResetInterval,
    ) -> Self {
        Self {
            amount_usd,
            reset_start_date: reset_start_date.into(),
            reset_interval,
        }
    }
}

/// A spending limit.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct SpendingLimit {
    /// The USD allowance per period.
    pub amount_usd: f64,
    /// The UTC `YYYY-MM-DD` day the schedule started.
    pub reset_start_date: String,
    /// How often the limit resets.
    pub reset_interval: ResetInterval,
}

/// The current period of a spending limit.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct SpendingLimitPeriod {
    /// When the period started.
    pub starts_at: Timestamp,
    /// When the period ends.
    pub ends_at: Timestamp,
    /// USD already spent.
    pub spent_usd: f64,
    /// USD reserved by in-flight work.
    pub reserved_usd: f64,
    /// USD still available.
    pub available_usd: f64,
}

/// A spending limit and its current period. With no limit set, every field is
/// `None`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct SpendingLimitStatus {
    /// The limit, or `None` when none is set.
    pub spending_limit: Option<SpendingLimit>,
    /// The current period, or `None` when no limit is set.
    pub period: Option<SpendingLimitPeriod>,
    /// When the next period starts, or `None` when no limit is set.
    pub next_reset_at: Option<Timestamp>,
}

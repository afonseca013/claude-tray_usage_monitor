mod antigravity;
mod claude;
mod openai;

pub use antigravity::AntigravityProvider;
pub use claude::ClaudeProvider;
pub use openai::OpenAiProvider;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UsageStatus {
    Ok,
    Warning,
    Rejected,
    Unavailable,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageSnapshot {
    pub provider: String,
    pub percent_5h: Option<f32>,
    pub percent_7d: Option<f32>,
    pub reset_5h: Option<i64>,
    pub reset_7d: Option<i64>,
    pub status: UsageStatus,
    /// Present when status is Unavailable/Error — shown as a tooltip in the UI.
    pub detail: Option<String>,
    pub fetched_at: i64,
}

impl UsageSnapshot {
    pub fn unavailable(provider: &str, now: i64, reason: &str) -> Self {
        Self {
            provider: provider.to_string(),
            percent_5h: None,
            percent_7d: None,
            reset_5h: None,
            reset_7d: None,
            status: UsageStatus::Unavailable,
            detail: Some(reason.to_string()),
            fetched_at: now,
        }
    }

    pub fn error(provider: &str, now: i64, reason: &str) -> Self {
        Self {
            provider: provider.to_string(),
            percent_5h: None,
            percent_7d: None,
            reset_5h: None,
            reset_7d: None,
            status: UsageStatus::Error,
            detail: Some(reason.to_string()),
            fetched_at: now,
        }
    }

    /// Highest of the 5h/7d percentages, used to pick tray icon color/threshold.
    pub fn worst_percent(&self) -> Option<f32> {
        match (self.percent_5h, self.percent_7d) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    }
}

#[async_trait::async_trait]
pub trait UsageProvider: Send + Sync {
    fn id(&self) -> &'static str;
    async fn fetch(&self) -> UsageSnapshot;
}

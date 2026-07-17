use crate::providers::UsageSnapshot;
use crate::storage::Storage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub poll_interval_minutes: u64,
    pub autostart: bool,
    #[serde(default = "default_true")]
    pub codex_enabled: bool,
    #[serde(default)]
    pub antigravity_enabled: bool,
}

fn default_true() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            poll_interval_minutes: 5,
            autostart: true,
            codex_enabled: true,
            // The local API it depends on currently 500s on every account
            // tested (see providers/antigravity.rs) — off by default until
            // that's understood, so the app isn't spawning a PowerShell
            // process every poll cycle for a provider that can't work yet.
            antigravity_enabled: false,
        }
    }
}

pub struct AppState {
    pub storage: Storage,
    pub latest: Mutex<HashMap<String, UsageSnapshot>>,
    pub settings: Mutex<Settings>,
    /// Highest usage threshold already notified for a given "{provider}:{window}"
    /// key (e.g. "claude:5h"), paired with the reset timestamp it applies to —
    /// a new reset timestamp means the window rolled over, so notifications
    /// start fresh for it.
    pub notified: Mutex<HashMap<String, (i64, u8)>>,
}

impl AppState {
    pub fn new(storage: Storage) -> Self {
        Self {
            storage,
            latest: Mutex::new(HashMap::new()),
            settings: Mutex::new(Settings::default()),
            notified: Mutex::new(HashMap::new()),
        }
    }
}

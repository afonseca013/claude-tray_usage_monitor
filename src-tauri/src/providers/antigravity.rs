use super::{UsageProvider, UsageSnapshot};
use chrono::Utc;

/// Google Antigravity does not expose a public API for quota/usage — it
/// consumes internal Google (Gemini/Vertex) quotas and only shows them
/// inside its own IDE UI. This stub keeps the card visible in Settings so
/// it can be wired up later without touching the rest of the app.
pub struct AntigravityProvider;

#[async_trait::async_trait]
impl UsageProvider for AntigravityProvider {
    fn id(&self) -> &'static str {
        "antigravity"
    }

    async fn fetch(&self) -> UsageSnapshot {
        UsageSnapshot::unavailable(
            self.id(),
            Utc::now().timestamp(),
            "Antigravity não expõe API pública de quota. Sem dados disponíveis.",
        )
    }
}

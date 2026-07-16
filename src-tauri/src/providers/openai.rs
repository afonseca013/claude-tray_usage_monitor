use super::{UsageProvider, UsageSnapshot, UsageStatus};
use crate::auth;
use chrono::Utc;

/// Only works if the user supplies an OpenAI Admin API key in Settings.
/// Reports token/cost consumption (not a "% of limit" — OpenAI's consumer
/// ChatGPT plans have no public quota-remaining endpoint), via the
/// organization usage endpoint.
pub struct OpenAiProvider;

const USAGE_URL: &str = "https://api.openai.com/v1/organization/usage/completions";

#[async_trait::async_trait]
impl UsageProvider for OpenAiProvider {
    fn id(&self) -> &'static str {
        "openai"
    }

    async fn fetch(&self) -> UsageSnapshot {
        let now = Utc::now().timestamp();

        let key = match auth::get_openai_admin_key() {
            Some(k) => k,
            None => {
                return UsageSnapshot::unavailable(
                    self.id(),
                    now,
                    "Sem API key de Admin da OpenAI configurada. ChatGPT web não tem endpoint público de quota.",
                )
            }
        };

        let start_time = now - 24 * 3600;
        let client = reqwest::Client::new();
        let result = client
            .get(USAGE_URL)
            .bearer_auth(&key)
            .query(&[("start_time", start_time.to_string()), ("bucket_width", "1d".to_string())])
            .send()
            .await;

        let response = match result {
            Ok(r) => r,
            Err(e) => return UsageSnapshot::error(self.id(), now, &format!("Falha de rede: {e}")),
        };

        if !response.status().is_success() {
            let code = response.status();
            return UsageSnapshot::error(self.id(), now, &format!("API respondeu {code} — verifique a API key."));
        }

        // The usage endpoint reports token counts, not a percent-of-limit.
        // We surface it as "status Ok, no percent" so the UI shows raw
        // numbers via `detail` instead of a progress ring.
        let body: serde_json::Value = match response.json().await {
            Ok(v) => v,
            Err(e) => return UsageSnapshot::error(self.id(), now, &format!("Resposta inesperada: {e}")),
        };

        let total_tokens: i64 = body["data"]
            .as_array()
            .map(|days| {
                days.iter()
                    .flat_map(|d| d["results"].as_array().cloned().unwrap_or_default())
                    .map(|r| r["input_tokens"].as_i64().unwrap_or(0) + r["output_tokens"].as_i64().unwrap_or(0))
                    .sum()
            })
            .unwrap_or(0);

        UsageSnapshot {
            provider: self.id().to_string(),
            percent_5h: None,
            percent_7d: None,
            reset_5h: None,
            reset_7d: None,
            status: UsageStatus::Ok,
            detail: Some(format!("{total_tokens} tokens nas últimas 24h")),
            fetched_at: now,
        }
    }
}

use super::{UsageProvider, UsageSnapshot, UsageStatus};
use crate::auth;
use chrono::Utc;

/// Reads Claude usage by piggy-backing on the rate-limit headers Anthropic
/// returns on every /v1/messages response — the same trick used by the
/// claude-usage-stick project. We send the smallest possible request
/// (max_tokens: 1) purely to read response headers, not the body.
///
/// NOTE: header names below match the documented unified rate-limit headers
/// as of the reference project's implementation. Validate against a live
/// response once a real token is configured — Anthropic may rename/add
/// headers over time.
pub struct ClaudeProvider;

const MESSAGES_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const OAUTH_BETA: &str = "oauth-2025-04-20";

#[async_trait::async_trait]
impl UsageProvider for ClaudeProvider {
    fn id(&self) -> &'static str {
        "claude"
    }

    async fn fetch(&self) -> UsageSnapshot {
        let now = Utc::now().timestamp();

        let token = match auth::get_claude_token() {
            Ok(Some(t)) => t,
            Ok(None) => {
                return UsageSnapshot::unavailable(
                    self.id(),
                    now,
                    "Token OAuth não configurado. Rode `claude setup-token` e cole o token nas Configurações.",
                )
            }
            Err(e) => {
                // The token IS saved — the OS credential store just couldn't
                // be read this cycle (locked session, Credential Manager
                // hiccup). Don't claim it's "not configured"; that sends
                // the user chasing a re-auth that isn't the problem.
                log::warn!("Falha lendo token do Claude no cofre de credenciais: {e}");
                return UsageSnapshot::unavailable(
                    self.id(),
                    now,
                    &format!("Não foi possível acessar o cofre de credenciais do Windows agora ({e}). Tentando novamente no próximo ciclo."),
                );
            }
        };

        let client = reqwest::Client::new();
        let result = client
            .post(MESSAGES_URL)
            .bearer_auth(&token)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("anthropic-beta", OAUTH_BETA)
            .json(&serde_json::json!({
                "model": "claude-haiku-4-5-20251001",
                "max_tokens": 1,
                "messages": [{"role": "user", "content": "ping"}]
            }))
            .send()
            .await;

        let response = match result {
            Ok(r) => r,
            Err(e) => return UsageSnapshot::error(self.id(), now, &format!("Falha de rede: {e}")),
        };

        let status_code = response.status();
        let headers = response.headers().clone();

        let get_f32 = |name: &str| -> Option<f32> {
            headers.get(name)?.to_str().ok()?.trim().parse::<f32>().ok()
        };
        let get_i64 = |name: &str| -> Option<i64> {
            headers.get(name)?.to_str().ok()?.trim().parse::<i64>().ok()
        };

        // Anthropic reports utilization as a 0.0-1.0 fraction, not 0-100.
        let percent_5h = get_f32("anthropic-ratelimit-unified-5h-utilization").map(|v| v * 100.0);
        let percent_7d = get_f32("anthropic-ratelimit-unified-7d-utilization").map(|v| v * 100.0);
        let reset_5h = get_i64("anthropic-ratelimit-unified-5h-reset");
        let reset_7d = get_i64("anthropic-ratelimit-unified-7d-reset");
        let ratelimit_status = headers
            .get("anthropic-ratelimit-unified-status")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        if !status_code.is_success() && percent_5h.is_none() && percent_7d.is_none() {
            let body_text = response.text().await.unwrap_or_default();

            // 529 ("overloaded_error") means Anthropic's API is temporarily
            // over capacity — transient, unrelated to the token. Surface it
            // as "unavailable" (gray icon) instead of "error" (red/blinking
            // critical icon), since the scheduler retries next poll anyway.
            if status_code.as_u16() == 529 || body_text.contains("overloaded_error") {
                log::warn!("Claude API sobrecarregada (529): {body_text}");
                return UsageSnapshot::unavailable(
                    self.id(),
                    now,
                    "Servidor da Anthropic sobrecarregado no momento — tentando novamente no próximo ciclo.",
                );
            }

            log::error!("Claude API error {status_code}: {body_text}");
            return UsageSnapshot::error(
                self.id(),
                now,
                &format!("API respondeu {status_code} — verifique o token. Detalhe: {body_text}"),
            );
        }

        let status = match ratelimit_status {
            "rejected" => UsageStatus::Rejected,
            "warning" => UsageStatus::Warning,
            _ => UsageStatus::Ok,
        };

        UsageSnapshot {
            provider: self.id().to_string(),
            percent_5h,
            percent_7d,
            reset_5h,
            reset_7d,
            status,
            detail: None,
            fetched_at: now,
        }
    }
}

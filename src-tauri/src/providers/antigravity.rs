use super::{UsageProvider, UsageSnapshot, UsageStatus};
use chrono::Utc;
use std::process::Stdio;
use tokio::process::Command;

/// Antigravity has no public quota API, but its background language server
/// (`language_server_windows_x64.exe`) exposes a local HTTPS/JSON-RPC port
/// authenticated by a CSRF token baked into its own command line — the same
/// mechanism the IDE's UI uses internally. We shell out to PowerShell to
/// find the process (WMI command-line + TCP listener lookup are much less
/// code than reimplementing that in Rust), then hit `GetUserStatus`
/// ourselves. The cert is self-signed, so TLS verification is disabled for
/// this one loopback call only.
pub struct AntigravityProvider;

const DISCOVER_SCRIPT: &str = r#"
$p = Get-CimInstance Win32_Process -Filter "name='language_server_windows_x64.exe'" |
    Where-Object { $_.CommandLine -notmatch '--enable_lsp' } |
    Select-Object -First 1
if (-not $p) { Write-Output '{}'; exit }
$null = $p.CommandLine -match '--csrf_token\s+(\S+)'
$token = $Matches[1]
$ports = Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue |
    Where-Object { $_.OwningProcess -eq $p.ProcessId } |
    Sort-Object LocalPort |
    Select-Object -ExpandProperty LocalPort
[PSCustomObject]@{ token = $token; ports = @($ports) } | ConvertTo-Json -Compress
"#;

#[async_trait::async_trait]
impl UsageProvider for AntigravityProvider {
    fn id(&self) -> &'static str {
        "antigravity"
    }

    async fn fetch(&self) -> UsageSnapshot {
        let now = Utc::now().timestamp();

        match tokio::time::timeout(std::time::Duration::from_secs(10), discover_and_query()).await {
            Ok(snapshot) => snapshot,
            Err(_) => UsageSnapshot::unavailable(
                self.id(),
                now,
                "Tempo esgotado consultando o language_server do Antigravity.",
            ),
        }
    }
}

async fn discover_and_query() -> UsageSnapshot {
    let now = Utc::now().timestamp();

    let mut command = Command::new("powershell");
    command
        .args(["-NoProfile", "-NonInteractive", "-Command", DISCOVER_SCRIPT])
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command
        .output()
        .await;

    let output = match output {
        Ok(o) => o,
        Err(e) => {
            return UsageSnapshot::unavailable(
                "antigravity",
                now,
                &format!("Falha ao consultar processos do Windows: {e}"),
            )
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(stdout.trim()) {
        Ok(v) => v,
        Err(_) => {
            return UsageSnapshot::unavailable(
                "antigravity",
                now,
                "Antigravity não está em execução.",
            )
        }
    };

    let token = match parsed.get("token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return UsageSnapshot::unavailable("antigravity", now, "Antigravity não está em execução."),
    };

    let ports: Vec<u64> = parsed
        .get("ports")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|p| p.as_u64()).collect())
        .unwrap_or_default();

    if ports.is_empty() {
        return UsageSnapshot::unavailable(
            "antigravity",
            now,
            "Antigravity está em execução, mas nenhuma porta local foi encontrada.",
        );
    }

    let client = match reqwest::Client::builder().danger_accept_invalid_certs(true).build() {
        Ok(c) => c,
        Err(e) => return UsageSnapshot::error("antigravity", now, &format!("Falha ao criar cliente HTTP: {e}")),
    };

    // The process listens on a few ports (extension server, LSP, etc.) —
    // only one of them speaks the ConnectRPC service we want, so probe each
    // until one returns a parseable JSON body (even an error body counts:
    // it means we reached the right multiplexer).
    for port in ports {
        let url = format!(
            "https://127.0.0.1:{port}/exa.language_server_pb.LanguageServerService/GetUserStatus"
        );
        let result = client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("X-Csrf-Token", &token)
            .header("X-Codeium-Csrf-Token", &token)
            .header("Connect-Protocol-Version", "1")
            .body("{}")
            .send()
            .await;

        let Ok(response) = result else { continue };
        let Ok(body) = response.json::<serde_json::Value>().await else { continue };

        if let Some(msg) = body.get("message").and_then(|v| v.as_str()) {
            // Reached the right port and got a well-formed ConnectRPC error —
            // Antigravity is definitely running, just not returning quota
            // data right now (observed: "GetCascadeModelConfigData() is nil"
            // on a freshly opened session with no active Cascade model).
            return UsageSnapshot {
                provider: "antigravity".to_string(),
                percent_5h: None,
                percent_7d: None,
                reset_5h: None,
                reset_7d: None,
                status: UsageStatus::Ok,
                detail: Some(format!("Antigravity ativo — API interna retornou erro: {msg}")),
                fetched_at: now,
            };
        }

        let prompt_credits = body
            .pointer("/userStatus/planStatus/availablePromptCredits")
            .and_then(|v| v.as_f64());
        let flow_credits = body
            .pointer("/userStatus/planStatus/availableFlowCredits")
            .and_then(|v| v.as_f64());

        if prompt_credits.is_some() || flow_credits.is_some() {
            let detail = match (prompt_credits, flow_credits) {
                (Some(p), Some(f)) => format!("Créditos — Prompt: {p:.0} · Flow: {f:.0}"),
                (Some(p), None) => format!("Créditos de prompt: {p:.0}"),
                (None, Some(f)) => format!("Créditos de flow: {f:.0}"),
                (None, None) => unreachable!(),
            };
            return UsageSnapshot {
                provider: "antigravity".to_string(),
                percent_5h: None,
                percent_7d: None,
                reset_5h: None,
                reset_7d: None,
                status: UsageStatus::Ok,
                detail: Some(detail),
                fetched_at: now,
            };
        }
    }

    UsageSnapshot::unavailable(
        "antigravity",
        now,
        "Antigravity está em execução, mas a API de status não respondeu em nenhuma porta.",
    )
}

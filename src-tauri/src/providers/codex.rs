use super::{UsageProvider, UsageSnapshot, UsageStatus};
use chrono::Utc;
use serde_json::{json, Value};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

/// Codex has no HTTP endpoint for rate-limit usage — it's only exposed via
/// the JSON-RPC protocol spoken by `codex app-server --stdio` (the same
/// backend the official Codex UI itself uses). We spawn that process fresh
/// on every poll, do the `initialize` handshake, ask for
/// `account/rateLimits/read`, then kill the process — mirroring the Node.js
/// reference implementation this was ported from.
pub struct CodexProvider;

const ID: &str = "codex";
const TIMEOUT: Duration = Duration::from_secs(15);

#[async_trait::async_trait]
impl UsageProvider for CodexProvider {
    fn id(&self) -> &'static str {
        ID
    }

    async fn fetch(&self) -> UsageSnapshot {
        match tokio::time::timeout(TIMEOUT, read_rate_limits()).await {
            Ok(Ok(snapshot)) => snapshot,
            Ok(Err(reason)) => UsageSnapshot::unavailable(ID, Utc::now().timestamp(), &reason),
            Err(_) => UsageSnapshot::error(
                ID,
                Utc::now().timestamp(),
                "Tempo esgotado aguardando resposta do codex app-server.",
            ),
        }
    }
}

async fn send(stdin: &mut tokio::process::ChildStdin, message: &Value) -> Result<(), String> {
    let mut line = serde_json::to_string(message).map_err(|e| e.to_string())?;
    line.push('\n');
    stdin.write_all(line.as_bytes()).await.map_err(|e| format!("Falha ao escrever para o codex app-server: {e}"))
}

async fn read_rate_limits() -> Result<UsageSnapshot, String> {
    let now = Utc::now().timestamp();
    let bin = std::env::var("CODEX_BIN").unwrap_or_else(|_| "codex".to_string());

    // The npm global install of the Codex CLI is a `.cmd` shim on Windows.
    // CreateProcess (what std::process::Command uses under the hood) can't
    // execute .cmd files directly — only cmd.exe knows how to run them — so
    // spawning "codex" straight fails silently as "not found" even though
    // `where codex` resolves it fine. Route through cmd.exe /C on Windows;
    // other platforms exec the binary directly.
    #[cfg(windows)]
    let mut command = {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut c = Command::new("cmd");
        c.args(["/C", &bin, "app-server", "--stdio"]);
        c.creation_flags(CREATE_NO_WINDOW);
        c
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut c = Command::new(&bin);
        c.args(["app-server", "--stdio"]);
        c
    };

    let mut child: Child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("Codex CLI não encontrado ({e}). Instale e autentique com `codex login`."))?;

    let mut stdin = child.stdin.take().expect("piped stdin");
    let stdout = child.stdout.take().expect("piped stdout");
    let mut lines = BufReader::new(stdout).lines();

    send(
        &mut stdin,
        &json!({
            "method": "initialize",
            "id": 1,
            "params": {
                "clientInfo": {
                    "name": "claude_token_monitor",
                    "title": "Claude Token Monitor",
                    "version": "1.0.0"
                }
            }
        }),
    )
    .await?;

    let response = loop {
        let line = lines
            .next_line()
            .await
            .map_err(|e| format!("Falha lendo o codex app-server: {e}"))?
            .ok_or_else(|| "codex app-server encerrou sem responder.".to_string())?;

        if line.trim().is_empty() {
            continue;
        }
        let message: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue, // ignore non-JSON-RPC noise on stdout
        };

        match message.get("id").and_then(Value::as_i64) {
            Some(1) => {
                send(&mut stdin, &json!({"method": "initialized", "params": {}})).await?;
                send(&mut stdin, &json!({"method": "account/rateLimits/read", "id": 2})).await?;
            }
            Some(2) => break message,
            _ => {}
        }
    };

    let _ = child.start_kill();

    let limits = response
        .get("result")
        .and_then(|r| r.get("rateLimits"))
        .ok_or_else(|| "Resposta do codex app-server sem rateLimits.".to_string())?;

    let window = |key: &str, field: &str| -> Option<f64> {
        limits.get(key)?.get(field)?.as_f64()
    };

    let percent_5h = window("primary", "usedPercent").map(|v| v as f32);
    let reset_5h = limits.get("primary").and_then(|w| w.get("resetsAt")).and_then(Value::as_i64);
    let percent_7d = window("secondary", "usedPercent").map(|v| v as f32);
    let reset_7d = limits.get("secondary").and_then(|w| w.get("resetsAt")).and_then(Value::as_i64);

    // Codex signals a hit limit via `rateLimitReachedType`; there's no
    // separate "warning" flag like Anthropic's header, so anything short of
    // an explicit reached-limit is reported Ok (the popup still colors bars
    // by percentage regardless of status).
    let status = match limits.get("rateLimitReachedType").and_then(Value::as_str) {
        Some(_) => UsageStatus::Rejected,
        None => UsageStatus::Ok,
    };

    Ok(UsageSnapshot {
        provider: ID.to_string(),
        percent_5h,
        percent_7d,
        reset_5h,
        reset_7d,
        status,
        detail: None,
        fetched_at: now,
    })
}

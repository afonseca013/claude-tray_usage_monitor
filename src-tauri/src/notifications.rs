use crate::providers::UsageSnapshot;
use crate::state::AppState;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

const THRESHOLDS: [u8; 5] = [25, 50, 75, 90, 100];

fn highest_crossed(percent: f32) -> u8 {
    THRESHOLDS
        .iter()
        .rev()
        .find(|&&t| percent >= t as f32)
        .copied()
        .unwrap_or(0)
}

/// Fires a toast for each provider window (5h/7d) that just crossed a new
/// 25/50/75/90/100% threshold since the last poll. Resets tracking whenever
/// the window's reset timestamp changes (i.e. the rate-limit window rolled
/// over), so the same thresholds can fire again next window.
pub fn check_and_notify(app: &AppHandle, snapshot: &UsageSnapshot) {
    let windows: [(&str, Option<f32>, Option<i64>); 2] = [
        ("5h", snapshot.percent_5h, snapshot.reset_5h),
        ("7d", snapshot.percent_7d, snapshot.reset_7d),
    ];

    for (label, percent, reset_at) in windows {
        let (Some(percent), Some(reset_at)) = (percent, reset_at) else { continue };
        let crossed = highest_crossed(percent);
        if crossed == 0 {
            continue;
        }

        let key = format!("{}:{}", snapshot.provider, label);
        let should_notify = {
            let state = app.state::<AppState>();
            let mut notified = state.notified.lock().expect("notified mutex poisoned");
            let entry = notified.entry(key).or_insert((reset_at, 0));
            if entry.0 != reset_at {
                *entry = (reset_at, 0);
            }
            if crossed > entry.1 {
                entry.1 = crossed;
                true
            } else {
                false
            }
        };

        if should_notify {
            let title = if crossed >= 100 {
                "Limite atingido"
            } else {
                "Uso de IA"
            };
            let body = format!(
                "{}: {crossed}% do limite de {label} atingido ({percent:.0}% atual).",
                provider_label(&snapshot.provider)
            );
            let _ = app.notification().builder().title(title).body(body).show();
        }
    }
}

fn provider_label(provider: &str) -> &str {
    match provider {
        "claude" => "Claude",
        "openai" => "ChatGPT",
        "antigravity" => "Antigravity",
        other => other,
    }
}

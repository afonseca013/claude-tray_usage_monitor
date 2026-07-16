use crate::providers::{AntigravityProvider, ClaudeProvider, OpenAiProvider, UsageProvider};
use crate::state::AppState;
use tauri::{AppHandle, Emitter, Manager};

const SEVEN_DAYS_SECS: i64 = 7 * 24 * 3600;

pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let providers: Vec<Box<dyn UsageProvider>> = vec![
            Box::new(ClaudeProvider),
            Box::new(OpenAiProvider),
            Box::new(AntigravityProvider),
        ];

        loop {
            run_once(&app, &providers).await;

            let interval_minutes = {
                let state = app.state::<AppState>();
                let settings = state.settings.lock().expect("settings mutex poisoned");
                settings.poll_interval_minutes.max(1)
            };
            tokio::time::sleep(std::time::Duration::from_secs(interval_minutes * 60)).await;
        }
    });
}

/// Triggered by the "refresh now" command from the UI — runs a single pass
/// outside of the regular polling cadence.
pub async fn run_once_now(app: &AppHandle) {
    let providers: Vec<Box<dyn UsageProvider>> = vec![
        Box::new(ClaudeProvider),
        Box::new(OpenAiProvider),
        Box::new(AntigravityProvider),
    ];
    run_once(app, &providers).await;
}

async fn run_once(app: &AppHandle, providers: &[Box<dyn UsageProvider>]) {
    let state = app.state::<AppState>();

    for provider in providers {
        let snapshot = provider.fetch().await;

        if let Err(e) = state.storage.insert(&snapshot) {
            log::error!("failed to store snapshot for {}: {e}", provider.id());
        }

        crate::notifications::check_and_notify(app, &snapshot);

        {
            let mut latest = state.latest.lock().expect("latest mutex poisoned");
            latest.insert(provider.id().to_string(), snapshot.clone());
        }

        let _ = app.emit("usage-updated", &snapshot);
    }

    let now = chrono::Utc::now().timestamp();
    let _ = state.storage.prune(now - SEVEN_DAYS_SECS);

    crate::tray::update_from_state(app);
}

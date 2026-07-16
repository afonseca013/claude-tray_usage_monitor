use crate::providers::UsageSnapshot;
use crate::state::{AppState, Settings};
use crate::{auth, scheduler};
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub fn get_latest_snapshots(state: State<AppState>) -> Vec<UsageSnapshot> {
    state
        .latest
        .lock()
        .expect("latest mutex poisoned")
        .values()
        .cloned()
        .collect()
}

#[tauri::command]
pub fn get_history(
    state: State<AppState>,
    provider: String,
    since_hours: i64,
) -> Result<Vec<(i64, Option<f32>, Option<f32>)>, String> {
    let since = chrono::Utc::now().timestamp() - since_hours * 3600;
    state.storage.history(&provider, since).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Settings {
    state.settings.lock().expect("settings mutex poisoned").clone()
}

#[tauri::command]
pub fn set_settings(state: State<AppState>, settings: Settings) {
    *state.settings.lock().expect("settings mutex poisoned") = settings;
}

#[tauri::command]
pub fn set_claude_token(token: String) -> Result<(), String> {
    auth::set_claude_token(&token)
}

#[tauri::command]
pub fn clear_claude_token() -> Result<(), String> {
    auth::clear_claude_token()
}

#[tauri::command]
pub fn has_claude_token() -> bool {
    auth::get_claude_token().is_some()
}

#[tauri::command]
pub fn set_openai_key(key: String) -> Result<(), String> {
    auth::set_openai_admin_key(&key)
}

#[tauri::command]
pub fn clear_openai_key() -> Result<(), String> {
    auth::clear_openai_admin_key()
}

#[tauri::command]
pub fn has_openai_key() -> bool {
    auth::get_openai_admin_key().is_some()
}

#[tauri::command]
pub async fn refresh_now(app: AppHandle) {
    scheduler::run_once_now(&app).await;
}

#[tauri::command]
pub fn hide_window(app: AppHandle, label: String) {
    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.hide();
    }
}

#[tauri::command]
pub fn show_settings_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

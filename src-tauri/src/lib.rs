mod auth;
mod commands;
mod notifications;
mod providers;
mod scheduler;
mod state;
mod storage;
mod tray;

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // A second launch just re-shows the popup instead of opening a
            // new instance.
            if let Some(window) = app.get_webview_window("popup") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_latest_snapshots,
            commands::get_history,
            commands::get_settings,
            commands::set_settings,
            commands::set_claude_token,
            commands::clear_claude_token,
            commands::has_claude_token,
            commands::set_openai_key,
            commands::clear_openai_key,
            commands::has_openai_key,
            commands::refresh_now,
            commands::hide_window,
            commands::show_settings_window,
        ])
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let storage = storage::Storage::new(&app_data_dir)?;
            app.manage(AppState::new(storage));

            tray::setup(app.handle())?;

            // Popup and settings windows hide instead of closing so their
            // webview state (and the tray icon) survive being dismissed.
            for label in ["popup", "settings"] {
                if let Some(window) = app.get_webview_window(label) {
                    let window_clone = window.clone();
                    window.on_window_event(move |event| {
                        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                            api.prevent_close();
                            let _ = window_clone.hide();
                        }
                    });
                }
            }

            // Popup hides itself when it loses focus, so clicking elsewhere
            // dismisses it like a native tray flyout.
            if let Some(popup) = app.get_webview_window("popup") {
                let popup_clone = popup.clone();
                popup.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        let _ = popup_clone.hide();
                    }
                });
            }

            scheduler::spawn(app.handle().clone());

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

use crate::providers::UsageStatus;
use crate::state::AppState;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};
use tauri_plugin_positioner::{Position, WindowExt};

const TRAY_ID: &str = "main-tray";

const ICON_OK: &[u8] = include_bytes!("../icons/tray/ok.png");
const ICON_WARNING: &[u8] = include_bytes!("../icons/tray/warning.png");
const ICON_REJECTED: &[u8] = include_bytes!("../icons/tray/rejected.png");
const ICON_REJECTED_DIM: &[u8] = include_bytes!("../icons/tray/rejected_dim.png");
const ICON_UNAVAILABLE: &[u8] = include_bytes!("../icons/tray/unavailable.png");

/// Threshold bands (worst 5h/7d percent) mapped to a tray icon color, mirroring
/// the claude-usage-stick's 25/50/70/100% animation cues.
#[derive(Clone, Copy, PartialEq)]
enum IconLevel {
    Ok,
    Warning,
    Critical,
    Unavailable,
}

fn icon_bytes(level: IconLevel, dim: bool) -> &'static [u8] {
    match (level, dim) {
        (IconLevel::Ok, _) => ICON_OK,
        (IconLevel::Warning, _) => ICON_WARNING,
        (IconLevel::Critical, false) => ICON_REJECTED,
        (IconLevel::Critical, true) => ICON_REJECTED_DIM,
        (IconLevel::Unavailable, _) => ICON_UNAVAILABLE,
    }
}

/// The tray icon API wants raw RGBA pixels, not encoded PNG bytes — decode
/// the embedded assets (they're tiny, 64x64, and baked in at compile time,
/// so a decode failure would mean a corrupt build, not a runtime concern).
fn decode_icon(bytes: &[u8]) -> Image<'static> {
    let decoded = image::load_from_memory(bytes)
        .expect("embedded tray icon asset is corrupt")
        .to_rgba8();
    let (width, height) = decoded.dimensions();
    Image::new_owned(decoded.into_raw(), width, height)
}

fn level_for(status: &UsageStatus, worst_percent: Option<f32>) -> IconLevel {
    if matches!(status, UsageStatus::Unavailable) {
        return IconLevel::Unavailable;
    }
    if matches!(status, UsageStatus::Rejected | UsageStatus::Error) {
        return IconLevel::Critical;
    }
    match worst_percent {
        Some(p) if p >= 70.0 => IconLevel::Critical,
        Some(p) if p >= 50.0 => IconLevel::Warning,
        Some(_) => IconLevel::Ok,
        None => IconLevel::Unavailable,
    }
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let open_item = MenuItem::with_id(app, "open", "Abrir", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "Configurações", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open_item, &settings_item, &quit_item])?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(decode_icon(ICON_UNAVAILABLE))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Uso de IA — carregando…")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => toggle_popup(app),
            "settings" => show_settings(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_popup(tray.app_handle());
            }
        })
        .build(app)?;

    spawn_blink_task(app.clone());

    Ok(())
}

fn toggle_popup(app: &AppHandle) {
    let Some(window) = app.get_webview_window("popup") else { return };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
    } else {
        let _ = window.move_window_constrained(Position::TrayCenter);
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn show_settings(app: &AppHandle) {
    let Some(window) = app.get_webview_window("settings") else { return };
    let _ = window.show();
    let _ = window.set_focus();
}

/// Refreshes the tray tooltip and icon color from the latest known
/// snapshots in `AppState`. The blink task (below) handles animating the
/// critical state; this just sets the "steady" (non-dim) icon.
pub fn update_from_state(app: &AppHandle) {
    let state = app.state::<AppState>();
    let latest = state.latest.lock().expect("latest mutex poisoned");

    let claude = latest.get("claude");
    let tooltip = match claude {
        Some(s) => match (s.percent_5h, s.percent_7d) {
            (Some(h5), Some(d7)) => format!("Claude: 5h {h5:.0}% · 7d {d7:.0}%"),
            (Some(h5), None) => format!("Claude: 5h {h5:.0}%"),
            (None, Some(d7)) => format!("Claude: 7d {d7:.0}%"),
            (None, None) => "Claude: sem dados".to_string(),
        },
        None => "Claude: sem dados".to_string(),
    };

    let level = claude
        .map(|s| level_for(&s.status, s.worst_percent()))
        .unwrap_or(IconLevel::Unavailable);
    drop(latest);

    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(tooltip.as_str()));
        if level != IconLevel::Critical {
            let _ = tray.set_icon(Some(decode_icon(icon_bytes(level, false))));
        }
    }
}

fn current_level(app: &AppHandle) -> IconLevel {
    let state = app.state::<AppState>();
    let latest = state.latest.lock().expect("latest mutex poisoned");
    latest
        .get("claude")
        .map(|s| level_for(&s.status, s.worst_percent()))
        .unwrap_or(IconLevel::Unavailable)
}

/// While Claude is at/above the "critical" threshold, pulses the tray icon
/// between full and dim red every 700ms so it's noticeable without polling.
fn spawn_blink_task(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut dim = false;
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(700)).await;

            if current_level(&app) != IconLevel::Critical {
                continue;
            }
            dim = !dim;

            if let Some(tray) = app.tray_by_id(TRAY_ID) {
                let _ = tray.set_icon(Some(decode_icon(icon_bytes(IconLevel::Critical, dim))));
            }
        }
    });
}

//! Commands about the app shell itself (menus, windows, System Settings deep links).

use tauri::AppHandle;

use crate::error::{AppError, CmdResult};

/// Called by the UI whenever a session starts or stops so File ▸ Start/Stop stay in sync.
#[tauri::command]
pub fn set_listening(app: AppHandle, live: bool) {
    crate::native::set_listening(&app, live);
}

#[tauri::command]
pub fn open_settings(app: AppHandle) {
    crate::native::open_settings(&app);
}

/// Deep link into System Settings ▸ Privacy & Security. `pane` is one of the Privacy anchors
/// (Microphone, Calendars) — anything else is rejected so this can't open arbitrary URLs.
#[tauri::command]
pub fn open_privacy_settings(pane: String) -> CmdResult<()> {
    const PANES: &[&str] = &["Microphone", "Calendars"];
    if !PANES.contains(&pane.as_str()) {
        return Err(AppError::invalid(format!("unknown privacy pane {pane}")));
    }
    let url = format!("x-apple.systempreferences:com.apple.preference.security?Privacy_{pane}");
    tauri_plugin_opener::open_url(url, None::<&str>)
        .map_err(|e| AppError { kind: crate::error::Kind::Internal, message: e.to_string() })
}

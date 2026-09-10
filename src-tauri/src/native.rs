//! The native macOS shell around the webview: menu bar, window lifecycle, Settings window.
//!
//! Everything a Mac user expects to find *outside* the web content lives here, so the React
//! side never has to fake it: the app/File/Edit/View/Window menus with their standard
//! shortcuts, hide-on-close with Dock reopen, "Float on Top", and a real Settings window.
//! Menu items the UI must react to are forwarded as a `menu` event carrying the item id.

use tauri::menu::{AboutMetadata, CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent, Wry};

pub const MAIN: &str = "main";
pub const SETTINGS: &str = "settings";

/// Menu ids the frontend listens for (see `src/hooks/useMenu.ts`).
pub mod id {
    pub const SETTINGS: &str = "settings";
    pub const START: &str = "start";
    pub const STOP: &str = "stop";
    pub const FIND: &str = "find";
    pub const GO_LIVE: &str = "go-live";
    pub const TOGGLE_SIDEBAR: &str = "toggle-sidebar";
    pub const TOGGLE_INSPECTOR: &str = "toggle-inspector";
    pub const FLOAT: &str = "float";
    pub const HELP: &str = "help";
}

/// Items whose state follows the session, kept so commands can flip them.
pub struct MenuHandles {
    pub start: MenuItem<Wry>,
    pub stop: MenuItem<Wry>,
    pub float: CheckMenuItem<Wry>,
}

pub fn build_menu(app: &AppHandle) -> tauri::Result<MenuHandles> {
    let pkg = app.package_info();
    let about = AboutMetadata {
        name: Some(pkg.name.clone()),
        version: Some(pkg.version.to_string()),
        copyright: Some("© Ed Knowles".into()),
        comments: Some(
            "Live translation, speaker detection and grammar coaching for video calls. Everything runs on this Mac."
                .into(),
        ),
        ..Default::default()
    };
    let app_menu = Submenu::with_items(
        app,
        &pkg.name,
        true,
        &[
            &PredefinedMenuItem::about(app, None, Some(about))?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, id::SETTINGS, "Settings…", true, Some("Cmd+,"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::services(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, None)?,
        ],
    )?;

    let start = MenuItem::with_id(app, id::START, "Start Listening", true, Some("Cmd+R"))?;
    let stop = MenuItem::with_id(app, id::STOP, "Stop Listening", false, Some("Cmd+."))?;
    let file = Submenu::with_items(
        app,
        "File",
        true,
        &[&start, &stop, &PredefinedMenuItem::separator(app)?, &PredefinedMenuItem::close_window(app, None)?],
    )?;

    let edit = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, id::FIND, "Find…", true, Some("Cmd+F"))?,
        ],
    )?;

    let view = Submenu::with_items(
        app,
        "View",
        true,
        &[
            &MenuItem::with_id(app, id::GO_LIVE, "Go to Live", true, Some("Cmd+1"))?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, id::TOGGLE_SIDEBAR, "Toggle Sidebar", true, Some("Ctrl+Cmd+S"))?,
            &MenuItem::with_id(app, id::TOGGLE_INSPECTOR, "Toggle Inspector", true, Some("Alt+Cmd+I"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::fullscreen(app, None)?,
        ],
    )?;

    let float = CheckMenuItem::with_id(app, id::FLOAT, "Float on Top", true, false, Some("Alt+Cmd+T"))?;
    let window = Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::maximize(app, Some("Zoom"))?,
            &PredefinedMenuItem::separator(app)?,
            &float,
        ],
    )?;

    let help = Submenu::with_items(
        app,
        "Help",
        true,
        &[&MenuItem::with_id(app, id::HELP, "LearnLive Help", true, None::<&str>)?],
    )?;

    let menu = Menu::with_items(app, &[&app_menu, &file, &edit, &view, &window, &help])?;
    app.set_menu(menu)?;
    Ok(MenuHandles { start, stop, float })
}

/// Menu items the shell handles itself; everything else goes to the main window as a `menu` event.
pub fn on_menu_event(app: &AppHandle, ev: MenuEvent) {
    use tauri::Emitter;
    match ev.id().as_ref() {
        id::SETTINGS => open_settings(app),
        id::FLOAT => {
            if let (Some(w), Some(h)) = (app.get_webview_window(MAIN), app.try_state::<MenuHandles>()) {
                let on = h.float.is_checked().unwrap_or(false);
                let _ = w.set_always_on_top(on);
            }
        }
        id::HELP => {
            let _ = tauri_plugin_opener::open_url("https://github.com/eknowles/learnlive#readme", None::<&str>);
        }
        other => {
            show_main(app);
            let _ = app.emit_to(MAIN, "menu", other.to_string());
        }
    }
}

/// Start/Stop menu items mirror the session, so the shortcuts never do the wrong thing.
pub fn set_listening(app: &AppHandle, live: bool) {
    if let Some(h) = app.try_state::<MenuHandles>() {
        let _ = h.start.set_enabled(!live);
        let _ = h.stop.set_enabled(live);
    }
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// One Settings window, focused if it already exists. The page decides what to render from
/// `location.hash` (see `src/main.tsx`).
pub fn open_settings(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(SETTINGS) {
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let r = WebviewWindowBuilder::new(app, SETTINGS, WebviewUrl::App("index.html#settings".into()))
        .title("Settings")
        .inner_size(560.0, 500.0)
        .resizable(false)
        .minimizable(false)
        .maximizable(false)
        .build();
    if let Err(e) = r {
        log::error!("settings window: {e}");
    }
}

/// Closing the main window hides it (the session keeps running); Dock click brings it back.
/// Closing Settings really closes it.
pub fn on_window_event(window: &tauri::Window, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        if window.label() == MAIN {
            let _ = window.hide();
            api.prevent_close();
        }
    }
}

pub fn on_run_event(app: &AppHandle, event: &RunEvent) {
    match event {
        RunEvent::Reopen { has_visible_windows: false, .. } => show_main(app),
        RunEvent::Exit => {
            // Quitting mid-session: stop cleanly so the meeting row is closed and voices folded in.
            if let Some(state) = app.try_state::<crate::AppState>() {
                crate::commands::session::shutdown(&state);
            }
        }
        _ => {}
    }
}

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod notes;

use db::DbState;
use std::path::Path;
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{command, AppHandle, Manager, State, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

/// Passed as a launch arg when the OS starts FlashPad automatically at login
/// (see the autostart plugin registration below), so `setup()` knows to keep
/// the window hidden in the tray instead of popping up on every login.
const HIDDEN_LAUNCH_FLAG: &str = "--hidden";

/// Fallback hotkey used until the user picks one in Settings.
const DEFAULT_HOTKEY: &str = "Alt+S";

/// Holds the file name (not path - joined with the app data dir at each use)
/// for persisting the user's chosen hotkey. Plain text, just the accelerator
/// string, since it's a single value - read in `setup()` so the hotkey is
/// registered natively before the frontend even loads.
const HOTKEY_FILE: &str = "hotkey.txt";

/// Tracks whichever hotkey is currently registered with the OS, so
/// `set_hotkey` knows what to unregister before registering a new one.
struct HotkeyState(Mutex<String>);

fn hotkey_file_path(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join(HOTKEY_FILE)
}

fn load_hotkey(data_dir: &Path) -> String {
    std::fs::read_to_string(hotkey_file_path(data_dir))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_HOTKEY.to_string())
}

#[command]
fn get_hotkey(state: State<HotkeyState>) -> String {
    state.0.lock().unwrap().clone()
}

#[command]
fn set_hotkey(app: AppHandle, state: State<HotkeyState>, hotkey: String) -> Result<(), String> {
    let trimmed = hotkey.trim();
    if trimmed.is_empty() {
        return Err("Hotkey cannot be empty".into());
    }

    let mut current = state.0.lock().map_err(|e| e.to_string())?;
    let shortcuts = app.global_shortcut();

    let _ = shortcuts.unregister(current.as_str());
    if let Err(err) = shortcuts.register(trimmed) {
        // Best-effort: put the old one back so the app isn't left without a
        // working hotkey.
        let _ = shortcuts.register(current.as_str());
        return Err(err.to_string());
    }

    *current = trimmed.to_string();

    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::write(hotkey_file_path(&data_dir), trimmed).map_err(|e| e.to_string())?;

    Ok(())
}

#[command]
fn hide_window(window: tauri::Window) {
    let _ = window.hide();
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Runs entirely in Rust so the hotkey works instantly even while the
/// webview is hidden - routing this through JS would require waking up a
/// potentially-suspended WebKitGTK process for what should be an instant
/// toggle, which was the cause of several-second delays after showing the
/// window via the hotkey.
fn toggle_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let visible = window.is_visible().unwrap_or(false);
        let focused = window.is_focused().unwrap_or(false);
        if visible && focused {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![HIDDEN_LAUNCH_FLAG.into()]),
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        toggle_main_window(app);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            hide_window,
            get_hotkey,
            set_hotkey,
            notes::list_notes,
            notes::create_note,
            notes::update_note,
            notes::delete_note,
            notes::move_note,
            notes::duplicate_note,
        ])
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");
            let conn = db::init(data_dir.join("flashpad.sqlite3"));
            app.manage(DbState(Mutex::new(conn)));

            let initial_hotkey = load_hotkey(&data_dir);
            app.global_shortcut()
                .register(initial_hotkey.as_str())
                .expect("failed to register initial hotkey");
            app.manage(HotkeyState(Mutex::new(initial_hotkey)));

            let open_item = MenuItem::with_id(app, "open", "Open FlashPad", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&open_item, &quit_item])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "open" => show_main_window(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main_window(tray.app_handle());
                    }
                })
                .build(app)?;

            if let Some(window) = app.get_webview_window("main") {
                let window_clone = window.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window_clone.hide();
                    }
                });

                if std::env::args().any(|arg| arg == HIDDEN_LAUNCH_FLAG) {
                    let _ = window.hide();
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

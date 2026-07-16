#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod backup;
mod db;
mod notes;
mod profiles;
mod scheduler;

use db::{DbState, DbStatusState};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{command, AppHandle, Manager, State, WindowEvent};

/// A dedicated rendering of the app mark for the tray, loaded explicitly
/// instead of reusing `app.default_window_icon()`.
const TRAY_ICON_BYTES: &[u8] = include_bytes!("../icons/tray-icon.png");
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

/// Tracks whether we last told the window to be shown or hidden. Every
/// hide/show trigger in this file (the hotkey, the tray icon, the
/// `hide_window` command used by Escape/minimize/close) goes through
/// `hide_main_window`/`show_main_window` below so this stays authoritative -
/// deliberately NOT re-derived from `window.is_visible()` on each call,
/// since that getter was observed to lag behind the real state under
/// XWayland (forced on Linux to fix the custom title bar, see GDK_BACKEND
/// below), which made the hotkey need an extra press to actually show the
/// window again after hiding it.
///
/// Starts `false` to match `visible: false` in tauri.conf.json - the window
/// is only ever actually shown once `frontend_ready` fires (see below).
static WINDOW_SHOWN: AtomicBool = AtomicBool::new(false);

/// Set once in `setup()` from the `--hidden` launch arg, so `frontend_ready`
/// knows whether this launch should stay backgrounded (autostart-at-login)
/// instead of popping the window up as soon as the frontend has rendered.
static LAUNCHED_HIDDEN: AtomicBool = AtomicBool::new(false);

fn hide_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
        WINDOW_SHOWN.store(false, Ordering::SeqCst);
    }
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
        WINDOW_SHOWN.store(true, Ordering::SeqCst);
    }
}

#[command]
fn hide_window(app: AppHandle) {
    hide_main_window(&app);
}

/// Called by the frontend once its initial render is actually ready to be
/// seen (data loaded, note selected/created, first paint done) - the window
/// is created invisible (`visible: false` in tauri.conf.json) specifically
/// so nothing is shown before this point. Showing eagerly at window-creation
/// time, before the webview had rendered anything, was causing a startup
/// flash of a blank/black window that then jumped to its restored
/// size/position and finally to the real UI once content painted.
#[command]
fn frontend_ready(app: AppHandle) {
    if !LAUNCHED_HIDDEN.load(Ordering::SeqCst) {
        show_main_window(&app);
    }
}

/// Runs entirely in Rust so the hotkey works instantly even while the
/// webview is hidden - routing this through JS would require waking up a
/// potentially-suspended WebKitGTK process for what should be an instant
/// toggle, which was the cause of several-second delays after showing the
/// window via the hotkey.
fn toggle_main_window(app: &tauri::AppHandle) {
    if WINDOW_SHOWN.load(Ordering::SeqCst) {
        hide_main_window(app);
    } else {
        show_main_window(app);
    }
}

fn main() {
    // KWin (KDE's Wayland compositor) draws its own server-side title bar
    // for GTK windows regardless of the app's `decorations: false` request -
    // that negotiation only works reliably over X11's Motif WM hints, not
    // natively over Wayland. Forcing GTK onto XWayland (still works fine
    // inside a native Wayland session) makes our custom title bar actually
    // take effect, with no per-user window-manager configuration needed.
    // Unconditional: Wayland desktop sessions commonly export GDK_BACKEND
    // themselves (e.g. GDK_BACKEND=wayland), so an "only if unset" check
    // never actually overrides it.
    #[cfg(target_os = "linux")]
    {
        std::env::set_var("GDK_BACKEND", "x11");
        eprintln!("[flashpad] GDK_BACKEND forced to: {:?}", std::env::var("GDK_BACKEND"));
    }

    tauri::Builder::default()
        .plugin(
            // Excludes DECORATIONS (see above) and VISIBLE: visibility is
            // entirely our own responsibility now (`frontend_ready`/
            // `LAUNCHED_HIDDEN` below) so the window stays invisible until
            // the frontend has actually rendered, rather than the plugin
            // restoring whatever visibility happened to be saved last time.
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::all()
                        & !tauri_plugin_window_state::StateFlags::DECORATIONS
                        & !tauri_plugin_window_state::StateFlags::VISIBLE,
                )
                .build(),
        )
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
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            hide_window,
            frontend_ready,
            get_hotkey,
            set_hotkey,
            notes::list_notes,
            notes::create_note,
            notes::update_note,
            notes::delete_note,
            notes::move_note,
            notes::duplicate_note,
            notes::reorder_note,
            profiles::get_app_state,
            profiles::list_databases,
            profiles::create_database,
            profiles::add_existing_database,
            profiles::rename_database,
            profiles::remove_database,
            profiles::switch_database,
            profiles::reload_database,
            profiles::set_database_path,
            profiles::get_backup_settings,
            profiles::set_backup_settings,
            backup::create_backup_now,
            backup::list_backups,
            backup::export_database,
            backup::import_database,
        ])
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");

            let mut config = profiles::load(&data_dir);
            // Guards against a hand-edited config.json whose active id
            // doesn't match any registered database - fall back to the
            // first profile rather than refusing to start.
            if profiles::active_database(&config).is_err() {
                if let Some(first) = config.databases.first() {
                    config.active_database_id = first.id;
                }
            }
            let active_path = profiles::active_database(&config)
                .expect("bootstrap always yields at least one database")
                .path
                .clone();
            let (conn, status) = db::activate(&active_path);

            app.manage(DbState(Mutex::new(conn)));
            app.manage(DbStatusState(Mutex::new(status)));
            app.manage(profiles::ConfigState(Mutex::new(config)));

            scheduler::spawn(app.handle().clone());

            let initial_hotkey = load_hotkey(&data_dir);
            app.global_shortcut()
                .register(initial_hotkey.as_str())
                .expect("failed to register initial hotkey");
            app.manage(HotkeyState(Mutex::new(initial_hotkey)));

            let open_item = MenuItem::with_id(app, "open", "Open FlashPad", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&open_item, &quit_item])?;

            let tray_icon = Image::from_bytes(TRAY_ICON_BYTES).expect("failed to decode tray icon");

            TrayIconBuilder::new()
                .icon(tray_icon)
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

            LAUNCHED_HIDDEN.store(
                std::env::args().any(|arg| arg == HIDDEN_LAUNCH_FLAG),
                Ordering::SeqCst,
            );

            if let Some(window) = app.get_webview_window("main") {
                let app_handle = app.handle().clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        hide_main_window(&app_handle);
                    }
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

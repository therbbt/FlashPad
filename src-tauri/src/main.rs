#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod backup;
mod db;
mod notes;
mod profiles;
mod scheduler;

use db::{DbState, DbStatusState};
use std::collections::HashSet;
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

/// Tracks two independent bits per utility window label ("settings",
/// "database-manager"): whether its frontend has actually painted
/// (`painted`), and whether something has asked for it to be seen
/// (`wants_visible`). The window is only ever shown once *both* are true -
/// see `maybe_show_utility_window`. Kept as two sets rather than one enum
/// map since either can flip independently and in either order: the app
/// pre-warms every utility window hidden at startup (painted flips first,
/// wants_visible stays false), while a normal cold open from a fresh click
/// sets wants_visible first and waits on painted.
struct UtilityWindowState {
    painted: Mutex<HashSet<String>>,
    wants_visible: Mutex<HashSet<String>>,
}

fn maybe_show_utility_window(app: &AppHandle, label: &str) {
    let state = app.state::<UtilityWindowState>();
    let painted = state.painted.lock().unwrap().contains(label);
    let wants_visible = state.wants_visible.lock().unwrap().contains(label);
    eprintln!("[flashpad] maybe_show_utility_window({label}): painted={painted} wants_visible={wants_visible}");
    if !(painted && wants_visible) {
        return;
    }
    match app.get_webview_window(label) {
        Some(window) => {
            if let Err(err) = window.show() {
                eprintln!("[flashpad] {label}.show() failed: {err}");
            }
            if let Err(err) = window.set_focus() {
                eprintln!("[flashpad] {label}.set_focus() failed: {err}");
            }
        }
        None => eprintln!("[flashpad] maybe_show_utility_window({label}): no such window"),
    }
}

/// Ensures `label` exists, creating it invisible and pre-loaded with
/// `index.html?window=<label>` if it doesn't (`main.ts` reads that query
/// param to decide which Svelte root to mount) - idempotent, so it's safe
/// to call both for startup pre-warming and from the on-demand open
/// commands below. Never shows the window itself: see
/// `maybe_show_utility_window`.
///
/// Closing one of these hides it rather than destroying it (mirroring the
/// main window's own close-to-tray behavior) specifically so the already-
/// warmed webview can be reused - recreating a whole webview from scratch
/// on every open was the main source of Settings/Database Manager feeling
/// slow to open.
fn open_utility_window(
    app: &AppHandle,
    label: &str,
    title: &str,
    width: f64,
    height: f64,
) -> Result<(), String> {
    if app.get_webview_window(label).is_some() {
        eprintln!("[flashpad] open_utility_window({label}): already exists, skipping create");
        return Ok(());
    }

    let url = format!("index.html?window={label}");
    eprintln!("[flashpad] open_utility_window({label}): creating, url={url}");

    tauri::WebviewWindowBuilder::new(app, label, tauri::WebviewUrl::App(url.into()))
        .title(title)
        .inner_size(width, height)
        .resizable(true)
        .transparent(true)
        .decorations(false)
        .visible(false)
        .build()
        .map_err(|e| {
            eprintln!("[flashpad] open_utility_window({label}): build failed: {e}");
            e.to_string()
        })?;

    if let Some(window) = app.get_webview_window(label) {
        let app_handle = app.clone();
        let label_owned = label.to_string();
        window.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if let Some(w) = app_handle.get_webview_window(&label_owned) {
                    let _ = w.hide();
                }
            }
        });
    }

    Ok(())
}

/// Pre-creates every utility window hidden, at app startup - so the first
/// time the user opens Settings or the Database Manager, the webview has
/// already been created and (usually) already painted, and only needs a
/// `.show()` rather than a full cold webview startup.
fn prewarm_utility_windows(app: &AppHandle) {
    let _ = open_utility_window(app, "settings", "FlashPad Settings", 460.0, 640.0);
    let _ = open_utility_window(app, "database-manager", "FlashPad Databases", 520.0, 560.0);
}

/// Marks `label` as wanted-visible, tries to show it now, and - in case the
/// frontend's own "I've painted" signal (`show_utility_window`) never
/// arrives for some reason (a JS error, a slow first paint, anything) -
/// schedules a fallback that force-shows it a couple seconds later
/// regardless. Better a rare early/unstyled reveal than a window that
/// silently never appears at all, which is strictly worse.
fn request_utility_window(app: &AppHandle, label: &'static str) {
    app.state::<UtilityWindowState>().wants_visible.lock().unwrap().insert(label.into());
    maybe_show_utility_window(app, label);

    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
        if let Some(window) = app_handle.get_webview_window(label) {
            if let Ok(false) = window.is_visible() {
                eprintln!("[flashpad] {label} still not visible after 2s, forcing show as a fallback");
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
    });
}

#[command]
fn open_settings_window(app: AppHandle) -> Result<(), String> {
    eprintln!("[flashpad] open_settings_window invoked");
    open_utility_window(&app, "settings", "FlashPad Settings", 460.0, 640.0)?;
    request_utility_window(&app, "settings");
    Ok(())
}

#[command]
fn open_database_window(app: AppHandle) -> Result<(), String> {
    eprintln!("[flashpad] open_database_window invoked");
    open_utility_window(&app, "database-manager", "FlashPad Databases", 520.0, 560.0)?;
    request_utility_window(&app, "database-manager");
    Ok(())
}

/// Called by a utility window's own frontend once its content has actually
/// painted with the right theme applied - see the doc comment on
/// `UtilityWindowState` for how this combines with `wants_visible` to
/// decide whether to actually reveal the window now.
#[command]
fn show_utility_window(app: AppHandle, label: String) -> Result<(), String> {
    eprintln!("[flashpad] show_utility_window({label}) invoked (frontend painted)");
    app.state::<UtilityWindowState>().painted.lock().unwrap().insert(label.clone());
    maybe_show_utility_window(&app, &label);
    Ok(())
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
        .invoke_handler(tauri::generate_handler![
            hide_window,
            frontend_ready,
            get_hotkey,
            set_hotkey,
            open_settings_window,
            open_database_window,
            show_utility_window,
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
            app.manage(UtilityWindowState {
                painted: Mutex::new(HashSet::new()),
                wants_visible: Mutex::new(HashSet::new()),
            });

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

            prewarm_utility_windows(&app.handle().clone());

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

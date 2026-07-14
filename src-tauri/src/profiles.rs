use crate::db::{self, DbState, DbStatus, DbStatusState};
use rusqlite::DatabaseName;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};

const CONFIG_FILE: &str = "flashpad.config.json";
const DEFAULT_DB_FILENAME: &str = "flashpad.sqlite3";
const DEFAULT_RETENTION_COUNT: u32 = 7;

/// Holds the app-level config (`flashpad.config.json`) - registered
/// databases, which one is active, and backup settings. Deliberately
/// separate from `DbState`: this survives even when the active database's
/// connection does not.
pub struct ConfigState(pub Mutex<AppConfig>);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseProfile {
    pub id: u32,
    pub name: String,
    pub path: PathBuf,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSettings {
    pub retention_count: u32,
}

impl Default for BackupSettings {
    fn default() -> Self {
        BackupSettings {
            retention_count: DEFAULT_RETENTION_COUNT,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub version: u32,
    pub next_id: u32,
    pub active_database_id: u32,
    pub databases: Vec<DatabaseProfile>,
    #[serde(default)]
    pub backup: BackupSettings,
}

fn now_iso() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

pub fn config_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(CONFIG_FILE)
}

/// Loads the app config, bootstrapping a default profile pointing at
/// FlashPad's historical hardcoded database path if no config exists yet
/// (or it fails to parse). This is what makes upgrading an existing
/// single-database install a complete no-op: the bootstrap only *describes*
/// where today's implicit database already lives - it never copies, moves,
/// or probes for the file's existence, since `db::open_and_migrate` already
/// handles both "file exists" and "fresh install" correctly on its own.
pub fn load(app_data_dir: &Path) -> AppConfig {
    let path = config_path(app_data_dir);
    if let Ok(contents) = std::fs::read_to_string(&path) {
        if let Ok(config) = serde_json::from_str::<AppConfig>(&contents) {
            return config;
        }
    }

    let default_profile = DatabaseProfile {
        id: 1,
        name: "Default".to_string(),
        path: app_data_dir.join(DEFAULT_DB_FILENAME),
        created_at: now_iso(),
    };
    let config = AppConfig {
        version: 1,
        next_id: 2,
        active_database_id: 1,
        databases: vec![default_profile],
        backup: BackupSettings::default(),
    };
    let _ = save(app_data_dir, &config);
    config
}

/// Atomic write: write to a temp file, then rename over the real path, so a
/// crash mid-write can never leave `flashpad.config.json` truncated/corrupt.
pub fn save(app_data_dir: &Path, config: &AppConfig) -> Result<(), String> {
    std::fs::create_dir_all(app_data_dir).map_err(|e| e.to_string())?;
    let path = config_path(app_data_dir);
    let tmp_path = path.with_extension("json.tmp");
    let contents = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(&tmp_path, contents).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp_path, &path).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn find_database(config: &AppConfig, id: u32) -> Result<&DatabaseProfile, String> {
    config
        .databases
        .iter()
        .find(|d| d.id == id)
        .ok_or_else(|| format!("No database with id {id}"))
}

pub fn active_database(config: &AppConfig) -> Result<&DatabaseProfile, String> {
    find_database(config, config.active_database_id)
}

pub fn add_database(config: &mut AppConfig, name: String, path: PathBuf) -> DatabaseProfile {
    let profile = DatabaseProfile {
        id: config.next_id,
        name,
        path,
        created_at: now_iso(),
    };
    config.next_id += 1;
    config.databases.push(profile.clone());
    profile
}

/// Named `_entry` to avoid clashing with the `rename_database` Tauri command
/// below, which wraps this plus a config save behind `State`/`AppHandle`.
pub fn rename_database_entry(config: &mut AppConfig, id: u32, name: String) -> Result<(), String> {
    let profile = config
        .databases
        .iter_mut()
        .find(|d| d.id == id)
        .ok_or_else(|| format!("No database with id {id}"))?;
    profile.name = name;
    Ok(())
}

/// Removes a database from the tracked list only - never touches the file
/// on disk. Refuses to remove the currently active profile or the last
/// remaining one, so the app can never end up with zero valid profiles.
/// Named `_entry` to avoid clashing with the `remove_database` Tauri command
/// below, which wraps this plus a config save behind `State`/`AppHandle`.
pub fn remove_database_entry(config: &mut AppConfig, id: u32) -> Result<(), String> {
    if config.databases.len() <= 1 {
        return Err("Cannot remove the only remaining database".to_string());
    }
    if config.active_database_id == id {
        return Err(
            "Cannot remove the currently active database - switch to another one first"
                .to_string(),
        );
    }
    let before = config.databases.len();
    config.databases.retain(|d| d.id != id);
    if config.databases.len() == before {
        return Err(format!("No database with id {id}"));
    }
    Ok(())
}

pub fn set_active(config: &mut AppConfig, id: u32) -> Result<(), String> {
    find_database(config, id)?;
    config.active_database_id = id;
    Ok(())
}

pub fn update_path_for(config: &mut AppConfig, id: u32, new_path: PathBuf) -> Result<(), String> {
    let profile = config
        .databases
        .iter_mut()
        .find(|d| d.id == id)
        .ok_or_else(|| format!("No database with id {id}"))?;
    profile.path = new_path;
    Ok(())
}

/// Pure string-matching heuristic used only to drive a UI warning banner
/// about sync-client corruption risk - never used to block anything, so
/// false positives/negatives are both acceptable.
pub fn looks_like_synced_folder(path: &Path) -> Option<&'static str> {
    let lower = path.to_string_lossy().to_lowercase();
    if lower.contains("onedrive") {
        Some("OneDrive")
    } else if lower.contains("dropbox") {
        Some("Dropbox")
    } else if lower.contains("google drive") || lower.contains("googledrive") {
        Some("Google Drive")
    } else if lower.contains("icloud drive") {
        Some("iCloud Drive")
    } else {
        None
    }
}

fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|e| e.to_string())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStateDto {
    pub active_database_id: u32,
    pub databases: Vec<DatabaseProfile>,
    pub ready: bool,
    pub error: Option<String>,
    pub sync_warning: Option<String>,
    pub backup: BackupSettings,
}

fn build_app_state(config: &AppConfig, status: &DbStatus) -> AppStateDto {
    let (ready, error) = match status {
        DbStatus::Ready => (true, None),
        DbStatus::Unavailable(reason) => (false, Some(reason.clone())),
    };
    let sync_warning = active_database(config)
        .ok()
        .and_then(|p| looks_like_synced_folder(&p.path))
        .map(|provider| {
            format!(
                "This database's folder appears to be synced by {provider}. FlashPad avoids WAL mode to reduce corruption risk, but avoid editing the same database from two devices at once."
            )
        });

    AppStateDto {
        active_database_id: config.active_database_id,
        databases: config.databases.clone(),
        ready,
        error,
        sync_warning,
        backup: config.backup.clone(),
    }
}

#[tauri::command]
pub fn get_app_state(
    config: State<ConfigState>,
    status: State<DbStatusState>,
) -> Result<AppStateDto, String> {
    let config = config.0.lock().map_err(|e| e.to_string())?;
    let status = status.0.lock().map_err(|e| e.to_string())?;
    Ok(build_app_state(&config, &status))
}

#[tauri::command]
pub fn list_databases(config: State<ConfigState>) -> Result<Vec<DatabaseProfile>, String> {
    Ok(config.0.lock().map_err(|e| e.to_string())?.databases.clone())
}

/// Creates a brand-new, empty database file at `path` and registers it as a
/// profile. Never touches the live `DbState` - only `switch_database` does -
/// so creating a profile the user doesn't immediately switch to can't
/// disrupt the session that's currently open.
#[tauri::command]
pub fn create_database(
    app: AppHandle,
    config: State<ConfigState>,
    name: String,
    path: String,
) -> Result<DatabaseProfile, String> {
    let path_buf = PathBuf::from(path);
    if path_buf.exists() {
        return Err("A file already exists at that location".to_string());
    }
    db::open_and_migrate(&path_buf)?;

    let mut config = config.0.lock().map_err(|e| e.to_string())?;
    let profile = add_database(&mut config, name, path_buf);
    save(&app_data_dir(&app)?, &config)?;
    Ok(profile)
}

/// Registers a pre-existing FlashPad database file as a profile, after
/// validating it's actually one (see `db::validate_flashpad_file`).
#[tauri::command]
pub fn add_existing_database(
    app: AppHandle,
    config: State<ConfigState>,
    name: String,
    path: String,
) -> Result<DatabaseProfile, String> {
    let path_buf = PathBuf::from(path);
    db::validate_flashpad_file(&path_buf)?;

    let mut config = config.0.lock().map_err(|e| e.to_string())?;
    let profile = add_database(&mut config, name, path_buf);
    save(&app_data_dir(&app)?, &config)?;
    Ok(profile)
}

#[tauri::command]
pub fn rename_database(
    app: AppHandle,
    config: State<ConfigState>,
    id: u32,
    name: String,
) -> Result<(), String> {
    let mut config = config.0.lock().map_err(|e| e.to_string())?;
    rename_database_entry(&mut config, id, name)?;
    save(&app_data_dir(&app)?, &config)
}

#[tauri::command]
pub fn remove_database(app: AppHandle, config: State<ConfigState>, id: u32) -> Result<(), String> {
    let mut config = config.0.lock().map_err(|e| e.to_string())?;
    remove_database_entry(&mut config, id)?;
    save(&app_data_dir(&app)?, &config)
}

/// Switches the live connection to a different registered profile without
/// restarting the app. Lock order is always `ConfigState` -> `DbState` ->
/// `DbStatusState` (held one at a time here, never nested) to avoid an
/// ordering deadlock against any other command that touches more than one
/// of these states.
#[tauri::command]
pub fn switch_database(
    app: AppHandle,
    config: State<ConfigState>,
    db_state: State<DbState>,
    status_state: State<DbStatusState>,
    id: u32,
) -> Result<AppStateDto, String> {
    let mut config = config.0.lock().map_err(|e| e.to_string())?;
    let profile = find_database(&config, id)?.clone();

    let (conn, new_status) = db::activate(&profile.path);
    *db_state.0.lock().map_err(|e| e.to_string())? = conn;
    *status_state.0.lock().map_err(|e| e.to_string())? = new_status;

    set_active(&mut config, id)?;
    save(&app_data_dir(&app)?, &config)?;

    let status = status_state.0.lock().map_err(|e| e.to_string())?;
    Ok(build_app_state(&config, &status))
}

/// Closes and reopens the active database's connection from its file on
/// disk, without changing which profile is active. Lets the user manually
/// pick up changes written by another device/process - e.g. a sync client
/// like OneDrive pulling down newer data written elsewhere - without
/// restarting the whole app. Same underlying `db::activate` used by
/// `switch_database`, just reactivating the current profile instead of a
/// different one.
#[tauri::command]
pub fn reload_database(
    config: State<ConfigState>,
    db_state: State<DbState>,
    status_state: State<DbStatusState>,
) -> Result<AppStateDto, String> {
    let config = config.0.lock().map_err(|e| e.to_string())?;
    let active_path = active_database(&config)?.path.clone();

    let (conn, new_status) = db::activate(&active_path);
    *db_state.0.lock().map_err(|e| e.to_string())? = conn;
    *status_state.0.lock().map_err(|e| e.to_string())? = new_status;

    let status = status_state.0.lock().map_err(|e| e.to_string())?;
    Ok(build_app_state(&config, &status))
}

/// Relocates the ACTIVE database to `new_path` without ever risking data
/// loss mid-move: copy (via the Online Backup API, not a raw file copy)
/// straight to the new location, open+validate that new file, only THEN
/// swap the live connection and persist the new path to config. The old
/// file is deliberately left in place - nothing is deleted - so there is
/// never a moment where neither copy is a verified, valid database.
#[tauri::command]
pub fn set_database_path(
    app: AppHandle,
    config: State<ConfigState>,
    db_state: State<DbState>,
    status_state: State<DbStatusState>,
    new_path: String,
) -> Result<DatabaseProfile, String> {
    let new_path_buf = PathBuf::from(&new_path);
    if new_path_buf.exists() {
        return Err("A file already exists at that location".to_string());
    }

    let mut config = config.0.lock().map_err(|e| e.to_string())?;
    let active_id = config.active_database_id;

    {
        let guard = db_state.0.lock().map_err(|e| e.to_string())?;
        let conn = guard.as_ref().ok_or("No database is currently available")?;
        if let Some(parent) = new_path_buf.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        conn.backup(DatabaseName::Main, &new_path_buf, None)
            .map_err(|e| e.to_string())?;
    }
    db::validate_flashpad_file(&new_path_buf)?;

    let (new_conn, new_status) = db::activate(&new_path_buf);
    *db_state.0.lock().map_err(|e| e.to_string())? = new_conn;
    *status_state.0.lock().map_err(|e| e.to_string())? = new_status;

    update_path_for(&mut config, active_id, new_path_buf)?;
    save(&app_data_dir(&app)?, &config)?;

    find_database(&config, active_id).map(|p| p.clone())
}

#[tauri::command]
pub fn get_backup_settings(config: State<ConfigState>) -> Result<BackupSettings, String> {
    Ok(config.0.lock().map_err(|e| e.to_string())?.backup.clone())
}

#[tauri::command]
pub fn set_backup_settings(
    app: AppHandle,
    config: State<ConfigState>,
    retention_count: u32,
) -> Result<(), String> {
    let mut config = config.0.lock().map_err(|e| e.to_string())?;
    config.backup.retention_count = retention_count;
    save(&app_data_dir(&app)?, &config)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("flashpad-profiles-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn load_bootstraps_default_profile_when_no_config_exists() {
        let dir = temp_dir("bootstrap");
        let config = load(&dir);

        assert_eq!(config.databases.len(), 1);
        assert_eq!(config.active_database_id, 1);
        assert_eq!(config.databases[0].name, "Default");
        assert_eq!(config.databases[0].path, dir.join("flashpad.sqlite3"));
        assert!(config_path(&dir).exists(), "bootstrap should persist the config immediately");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = temp_dir("roundtrip");
        let mut config = load(&dir);
        add_database(&mut config, "Work".to_string(), dir.join("work.sqlite3"));
        save(&dir, &config).unwrap();

        let reloaded = load(&dir);
        assert_eq!(reloaded.databases.len(), 2);
        assert_eq!(reloaded.databases[1].name, "Work");
        assert_eq!(reloaded.next_id, 3);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rename_and_switch_active_database() {
        let dir = temp_dir("rename-switch");
        let mut config = load(&dir);
        let work = add_database(&mut config, "Work".to_string(), dir.join("work.sqlite3"));

        rename_database_entry(&mut config, work.id, "Work Notes".to_string()).unwrap();
        assert_eq!(find_database(&config, work.id).unwrap().name, "Work Notes");

        set_active(&mut config, work.id).unwrap();
        assert_eq!(active_database(&config).unwrap().id, work.id);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cannot_remove_active_database() {
        let dir = temp_dir("remove-active");
        let mut config = load(&dir);
        add_database(&mut config, "Work".to_string(), dir.join("work.sqlite3"));

        let result = remove_database_entry(&mut config, 1);
        assert!(result.is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cannot_remove_last_remaining_database() {
        let dir = temp_dir("remove-last");
        let mut config = load(&dir);
        set_active(&mut config, 1).unwrap();

        let result = remove_database_entry(&mut config, 1);
        assert!(result.is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn remove_database_deletes_list_entry_only_not_the_file() {
        let dir = temp_dir("remove-entry");
        let mut config = load(&dir);
        let work_path = dir.join("work.sqlite3");
        std::fs::write(&work_path, b"not a real db, just proving it's untouched").unwrap();
        let work = add_database(&mut config, "Work".to_string(), work_path.clone());

        remove_database_entry(&mut config, work.id).unwrap();

        assert_eq!(config.databases.len(), 1);
        assert!(work_path.exists(), "removing a profile must never delete its file");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn looks_like_synced_folder_detects_common_providers() {
        assert_eq!(
            looks_like_synced_folder(Path::new("/home/user/OneDrive/flashpad.sqlite3")),
            Some("OneDrive")
        );
        assert_eq!(
            looks_like_synced_folder(Path::new("/home/user/Dropbox/notes/flashpad.sqlite3")),
            Some("Dropbox")
        );
        assert_eq!(
            looks_like_synced_folder(Path::new("/home/user/Documents/flashpad.sqlite3")),
            None
        );
    }

    // The following three mirror the exact scenarios the implementation plan
    // calls for testing at app boot (fresh install / existing pre-upgrade
    // install / a broken path), exercised directly against `profiles::load`
    // + `db::activate` since there's no display server in this environment
    // to drive the real Tauri window for a manual end-to-end pass.

    #[test]
    fn boot_fresh_install_bootstraps_and_activates_cleanly() {
        let dir = temp_dir("boot-fresh");
        let config = load(&dir);
        let active = active_database(&config).unwrap();

        let (conn, status) = db::activate(&active.path);
        assert!(conn.is_some(), "a fresh install should open successfully");
        assert!(matches!(status, DbStatus::Ready));
        assert!(active.path.exists(), "open_and_migrate should have created the file");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn boot_existing_preupgrade_install_finds_the_real_file_without_copying() {
        let dir = temp_dir("boot-existing");
        // Simulate a pre-upgrade install: a real database already sitting at
        // today's hardcoded path, but no config.json yet.
        let existing_conn = db::open_and_migrate(&dir.join("flashpad.sqlite3")).unwrap();
        existing_conn
            .execute(
                "INSERT INTO notes (title, content, created_at, updated_at) VALUES ('Pre-existing note', '', '2026-01-01T00:00:00', '2026-01-01T00:00:00')",
                [],
            )
            .unwrap();
        drop(existing_conn);

        assert!(!config_path(&dir).exists(), "no config.json should exist yet, matching a real pre-upgrade install");

        let config = load(&dir);
        let active = active_database(&config).unwrap();
        assert_eq!(active.path, dir.join("flashpad.sqlite3"));

        let (conn, status) = db::activate(&active.path);
        assert!(matches!(status, DbStatus::Ready));
        let conn = conn.unwrap();
        let note_count: i64 = conn.query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0)).unwrap();
        assert_eq!(note_count, 1, "the user's existing note must survive the upgrade untouched");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn boot_falls_back_gracefully_when_configured_path_is_unreachable() {
        let dir = temp_dir("boot-broken");
        // A file (not a directory) standing where the database's parent
        // directory needs to be makes `create_dir_all` fail - simulating an
        // unmounted sync folder without needing a real mount point.
        let blocked_parent = dir.join("not-a-directory");
        std::fs::write(&blocked_parent, b"blocking file").unwrap();
        let unreachable_path = blocked_parent.join("flashpad.sqlite3");

        let (conn, status) = db::activate(&unreachable_path);
        assert!(conn.is_none(), "an unreachable path must never yield a placeholder connection");
        assert!(matches!(status, DbStatus::Unavailable(_)), "must report Unavailable, not panic");

        let _ = std::fs::remove_dir_all(&dir);
    }
}

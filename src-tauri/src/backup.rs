use crate::db::{self, DbState};
use crate::profiles::ConfigState;
use rusqlite::backup::{Backup, Progress};
use rusqlite::{Connection, DatabaseName};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Manager, State};

const BACKUP_PREFIX: &str = "flashpad-backup-";
const SAFETY_PREFIX: &str = "flashpad-preimport-";
const BACKUP_EXT: &str = "db";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub path: PathBuf,
    pub filename: String,
    pub size_bytes: u64,
}

pub fn backups_dir(app_data_dir: &Path, database_id: u32) -> PathBuf {
    app_data_dir.join("backups").join(database_id.to_string())
}

/// Copies `conn`'s data into a fresh file at `dest_path` using SQLite's
/// Online Backup API (page-by-page, lock-aware, safe against a writer on
/// `conn` mid-copy) rather than a raw file copy - the direct answer to the
/// "sync client copies mid-write and corrupts the file" risk. Uses
/// `run_to_completion` (not the `Connection::backup` convenience wrapper,
/// which gives up immediately on `Busy`/`Locked`) since this also runs
/// unattended from the scheduler.
fn run_backup_to(conn: &Connection, dest_path: &Path) -> Result<(), String> {
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut dest_conn = Connection::open(dest_path).map_err(|e| e.to_string())?;
    let backup = Backup::new(conn, &mut dest_conn).map_err(|e| e.to_string())?;
    backup
        .run_to_completion(100, Duration::from_millis(250), None)
        .map_err(|e| e.to_string())
}

/// Once-daily backup; filename encodes only the date, so a same-day rerun
/// (e.g. a manual "back up now" on a day the scheduler already ran)
/// overwrites rather than accumulating duplicates.
pub fn create_backup(conn: &Connection, dir: &Path) -> Result<PathBuf, String> {
    let filename = format!(
        "{BACKUP_PREFIX}{}.{BACKUP_EXT}",
        chrono::Local::now().format("%Y-%m-%d")
    );
    let path = dir.join(filename);
    run_backup_to(conn, &path)?;
    Ok(path)
}

/// Full-timestamp safety backup taken immediately before a destructive
/// import. Deliberately a different filename family from `create_backup` so
/// `prune_backups` (which only ever globs `flashpad-backup-*`) can never
/// delete the one backup protecting against an in-progress restore.
pub fn create_safety_backup(conn: &Connection, dir: &Path) -> Result<PathBuf, String> {
    let filename = format!(
        "{SAFETY_PREFIX}{}.{BACKUP_EXT}",
        chrono::Local::now().format("%Y-%m-%dT%H%M%S")
    );
    let path = dir.join(filename);
    run_backup_to(conn, &path)?;
    Ok(path)
}

/// (date, path) pairs for every `flashpad-backup-*` file in `dir`, sorted
/// oldest first. An unreadable/missing `dir` is treated as "no backups yet",
/// not an error - the scheduler and retention pruning both rely on this.
fn dated_backups(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<(String, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?.to_string();
            let date = name
                .strip_prefix(BACKUP_PREFIX)?
                .strip_suffix(&format!(".{BACKUP_EXT}"))?
                .to_string();
            Some((date, path))
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Deletes the oldest `flashpad-backup-*` files beyond `keep`. Never touches
/// `flashpad-preimport-*` safety backups.
pub fn prune_backups(dir: &Path, keep: u32) -> Result<(), String> {
    let dates = dated_backups(dir);
    let keep = keep as usize;
    if dates.len() <= keep {
        return Ok(());
    }
    let overflow = dates.len() - keep;
    for (_, path) in dates.into_iter().take(overflow) {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// All backups (daily + safety) in `dir`, for display in Settings.
pub fn list_backup_files(dir: &Path) -> Vec<BackupInfo> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut backups: Vec<BackupInfo> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let filename = path.file_name()?.to_str()?.to_string();
            if !filename.starts_with(BACKUP_PREFIX) && !filename.starts_with(SAFETY_PREFIX) {
                return None;
            }
            let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
            Some(BackupInfo {
                path,
                filename,
                size_bytes,
            })
        })
        .collect();
    backups.sort_by(|a, b| b.filename.cmp(&a.filename));
    backups
}

/// The date of the newest `flashpad-backup-*` file in `dir`, if any. Derived
/// straight from filenames rather than tracked separately in config, so
/// there's nothing to keep in sync.
pub fn last_backup_at(dir: &Path) -> Option<chrono::NaiveDate> {
    dated_backups(dir)
        .last()
        .and_then(|(date, _)| chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
}

fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|e| e.to_string())
}

fn with_active_connection<T>(
    db: &State<DbState>,
    f: impl FnOnce(&Connection) -> Result<T, String>,
) -> Result<T, String> {
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No database is currently available")?;
    f(conn)
}

#[tauri::command]
pub fn create_backup_now(
    app: AppHandle,
    db: State<DbState>,
    config: State<ConfigState>,
) -> Result<BackupInfo, String> {
    let active_id = config.0.lock().map_err(|e| e.to_string())?.active_database_id;
    let dir = backups_dir(&app_data_dir(&app)?, active_id);

    let path = with_active_connection(&db, |conn| create_backup(conn, &dir))?;
    let size_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let filename = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    Ok(BackupInfo {
        path,
        filename,
        size_bytes,
    })
}

#[tauri::command]
pub fn list_backups(app: AppHandle, config: State<ConfigState>) -> Result<Vec<BackupInfo>, String> {
    let active_id = config.0.lock().map_err(|e| e.to_string())?.active_database_id;
    let dir = backups_dir(&app_data_dir(&app)?, active_id);
    Ok(list_backup_files(&dir))
}

/// Exports the active database to an arbitrary user-chosen path (picked via
/// the native save dialog on the frontend). Interactive and immediately
/// reported to the user, so the simpler `Connection::backup` convenience
/// wrapper (no busy-retry) is fine here, unlike the unattended scheduler.
#[tauri::command]
pub fn export_database(db: State<DbState>, dest_path: String) -> Result<(), String> {
    with_active_connection(&db, |conn| {
        conn.backup(DatabaseName::Main, &dest_path, None)
            .map_err(|e| e.to_string())
    })
}

/// Replaces the active database's contents with `source_path`'s, after
/// validating it's a real FlashPad file and taking a safety backup of the
/// current data first. The safety backup happens strictly before the
/// destructive restore, and uses the `flashpad-preimport-*` filename family
/// so `prune_backups`'s retention logic (which only ever globs
/// `flashpad-backup-*`) can never delete it out from under an in-progress
/// import. Restores directly onto the live connection via SQLite's Online
/// Backup API (`Connection::restore`, the mirror of `Connection::backup`
/// used for export) rather than swapping files on disk, then re-runs the
/// migration chain in case the imported file predates one.
#[tauri::command]
pub fn import_database(
    app: AppHandle,
    db: State<DbState>,
    config: State<ConfigState>,
    source_path: String,
) -> Result<(), String> {
    let source = PathBuf::from(&source_path);
    db::validate_flashpad_file(&source)?;

    let active_id = config.0.lock().map_err(|e| e.to_string())?.active_database_id;
    let dir = backups_dir(&app_data_dir(&app)?, active_id);

    let mut guard = db.0.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_mut().ok_or("No database is currently available")?;

    create_safety_backup(conn, &dir)?;

    conn.restore(DatabaseName::Main, &source, None::<fn(Progress)>)
        .map_err(|e| e.to_string())?;

    db::ensure_schema(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exercises the same sequence `import_database` performs (safety
    /// backup, then restore, then ensure_schema) directly against rusqlite,
    /// without needing Tauri's `State` wiring - proving the ordering itself
    /// is correct: the safety backup must contain the pre-import data, and
    /// the live connection must contain the imported data afterward.
    #[test]
    fn safety_backup_then_restore_preserves_old_data_and_applies_new() {
        let dir = temp_dir("import-flow");

        let mut active_conn = seeded_conn(); // has a note titled "Hello"
        let source_path = dir.join("source.sqlite3");
        let source_conn = Connection::open(&source_path).unwrap();
        source_conn
            .execute_batch(
                "CREATE TABLE notes (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    title TEXT NOT NULL DEFAULT 'Untitled',
                    content TEXT NOT NULL DEFAULT '',
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    parent_id INTEGER REFERENCES notes(id) ON DELETE CASCADE
                );
                INSERT INTO notes (title, content, created_at, updated_at)
                VALUES ('Imported note', '', '2026-01-01T00:00:00', '2026-01-01T00:00:00');",
            )
            .unwrap();
        drop(source_conn);

        let safety_dir = dir.join("safety");
        let safety_path = create_safety_backup(&active_conn, &safety_dir).unwrap();

        active_conn
            .restore(DatabaseName::Main, &source_path, None::<fn(Progress)>)
            .unwrap();
        db::ensure_schema(&active_conn).unwrap();

        let safety_conn = Connection::open(&safety_path).unwrap();
        let safety_title: String = safety_conn
            .query_row("SELECT title FROM notes WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(safety_title, "Hello", "safety backup must hold the pre-import data");

        let active_title: String = active_conn
            .query_row("SELECT title FROM notes WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(active_title, "Imported note", "live connection must hold the imported data");

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "flashpad-backup-test-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn seeded_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE notes (id INTEGER PRIMARY KEY, title TEXT);
             INSERT INTO notes (title) VALUES ('Hello');",
        )
        .unwrap();
        conn
    }

    #[test]
    fn create_backup_produces_a_readable_copy() {
        let dir = temp_dir("create");
        let conn = seeded_conn();

        let path = create_backup(&conn, &dir).unwrap();
        assert!(path.exists());

        let restored = Connection::open(&path).unwrap();
        let title: String = restored
            .query_row("SELECT title FROM notes WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(title, "Hello");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn create_backup_twice_same_day_overwrites_not_duplicates() {
        let dir = temp_dir("overwrite");
        let conn = seeded_conn();

        create_backup(&conn, &dir).unwrap();
        create_backup(&conn, &dir).unwrap();

        assert_eq!(dated_backups(&dir).len(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn prune_backups_keeps_only_the_newest_n() {
        let dir = temp_dir("prune");
        for date in ["2026-07-01", "2026-07-02", "2026-07-03", "2026-07-04"] {
            std::fs::write(dir.join(format!("{BACKUP_PREFIX}{date}.{BACKUP_EXT}")), b"x").unwrap();
        }

        prune_backups(&dir, 2).unwrap();

        let remaining = dated_backups(&dir);
        assert_eq!(remaining.len(), 2);
        assert_eq!(remaining[0].0, "2026-07-03");
        assert_eq!(remaining[1].0, "2026-07-04");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn prune_backups_never_touches_safety_backups() {
        let dir = temp_dir("prune-safety");
        std::fs::write(dir.join(format!("{BACKUP_PREFIX}2026-07-01.{BACKUP_EXT}")), b"x").unwrap();
        std::fs::write(
            dir.join(format!("{SAFETY_PREFIX}2026-07-01T120000.{BACKUP_EXT}")),
            b"x",
        )
        .unwrap();

        prune_backups(&dir, 0).unwrap();

        let files = list_backup_files(&dir);
        assert_eq!(files.len(), 1);
        assert!(files[0].filename.starts_with(SAFETY_PREFIX));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn last_backup_at_reads_the_newest_filename_date() {
        let dir = temp_dir("last-backup-at");
        assert_eq!(last_backup_at(&dir), None, "no backups yet");

        std::fs::write(dir.join(format!("{BACKUP_PREFIX}2026-07-01.{BACKUP_EXT}")), b"x").unwrap();
        std::fs::write(dir.join(format!("{BACKUP_PREFIX}2026-07-10.{BACKUP_EXT}")), b"x").unwrap();

        assert_eq!(
            last_backup_at(&dir),
            chrono::NaiveDate::from_ymd_opt(2026, 7, 10)
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn list_backup_files_on_missing_dir_returns_empty_not_error() {
        let dir = std::env::temp_dir().join("flashpad-backup-test-does-not-exist");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(list_backup_files(&dir).is_empty());
    }
}

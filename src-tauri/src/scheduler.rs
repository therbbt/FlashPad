use crate::backup;
use crate::db;
use crate::profiles::{ConfigState, DatabaseProfile};
use chrono::Local;
use std::path::Path;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Manager};

const CHECK_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// Spawns one background thread that runs a backup check immediately -
/// covering "check on startup" - and then every hour for as long as the app
/// runs, including while it sits tray-only for days with no window ever
/// rendered. This is the only place daily backups are driven from; nothing
/// on the frontend is involved.
pub fn spawn(app: AppHandle) {
    thread::spawn(move || loop {
        run_check(&app);
        thread::sleep(CHECK_INTERVAL);
    });
}

fn run_check(app: &AppHandle) {
    let Some(config_state) = app.try_state::<ConfigState>() else {
        return;
    };
    let Ok(app_data_dir) = app.path().app_data_dir() else {
        return;
    };
    let (databases, retention_count) = {
        let Ok(config) = config_state.0.lock() else {
            return;
        };
        (config.databases.clone(), config.backup.retention_count)
    };

    run_check_for(&databases, &app_data_dir, retention_count);
}

/// Backs up every profile in `databases` whose newest backup isn't from
/// today, using a fresh short-lived connection opened directly via
/// `db::open_and_migrate` rather than reaching into the UI thread's live
/// `DbState` mutex. Applies to every registered database, not just the
/// active one, per the "daily backups apply per database" requirement -
/// profiles that are never switched into still get backed up. A profile
/// whose path is currently unreachable is logged and skipped, never panics
/// the thread.
fn run_check_for(databases: &[DatabaseProfile], app_data_dir: &Path, retention_count: u32) {
    let today = Local::now().date_naive();

    for profile in databases {
        let dir = backup::backups_dir(app_data_dir, profile.id);
        if backup::last_backup_at(&dir) == Some(today) {
            continue;
        }

        match db::open_and_migrate(&profile.path) {
            Ok(conn) => {
                if let Err(err) = backup::create_backup(&conn, &dir) {
                    eprintln!(
                        "[flashpad-backup] failed to back up '{}': {err}",
                        profile.name
                    );
                    continue;
                }
                if let Err(err) = backup::prune_backups(&dir, retention_count) {
                    eprintln!(
                        "[flashpad-backup] failed to prune backups for '{}': {err}",
                        profile.name
                    );
                }
            }
            Err(err) => {
                eprintln!(
                    "[flashpad-backup] skipping '{}', path unreachable: {err}",
                    profile.name
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration as ChronoDuration;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "flashpad-scheduler-test-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn profile(id: u32, path: std::path::PathBuf) -> DatabaseProfile {
        DatabaseProfile {
            id,
            name: format!("Profile {id}"),
            path,
            created_at: "2026-01-01T00:00:00".to_string(),
        }
    }

    #[test]
    fn backs_up_a_profile_with_no_prior_backup() {
        let dir = temp_dir("no-prior");
        let db_path = dir.join("a.sqlite3");
        db::open_and_migrate(&db_path).unwrap();
        let profiles = vec![profile(1, db_path)];

        run_check_for(&profiles, &dir, 7);

        let backups_dir = backup::backups_dir(&dir, 1);
        assert_eq!(backup::list_backup_files(&backups_dir).len(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn skips_a_profile_already_backed_up_today() {
        let dir = temp_dir("already-today");
        let db_path = dir.join("a.sqlite3");
        db::open_and_migrate(&db_path).unwrap();
        let profiles = vec![profile(1, db_path)];
        let backups_dir = backup::backups_dir(&dir, 1);

        run_check_for(&profiles, &dir, 7);
        let first_run_files = backup::list_backup_files(&backups_dir);
        assert_eq!(first_run_files.len(), 1);

        run_check_for(&profiles, &dir, 7);
        let second_run_files = backup::list_backup_files(&backups_dir);
        assert_eq!(second_run_files.len(), 1, "a same-day rerun must not create a second file");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn backs_up_a_profile_whose_newest_backup_is_from_yesterday() {
        let dir = temp_dir("yesterday");
        let db_path = dir.join("a.sqlite3");
        db::open_and_migrate(&db_path).unwrap();
        let profiles = vec![profile(1, db_path)];
        let backups_dir = backup::backups_dir(&dir, 1);
        std::fs::create_dir_all(&backups_dir).unwrap();

        let yesterday = Local::now().date_naive() - ChronoDuration::days(1);
        std::fs::write(
            backups_dir.join(format!("flashpad-backup-{yesterday}.db")),
            b"stale",
        )
        .unwrap();

        run_check_for(&profiles, &dir, 7);

        assert_eq!(
            backup::last_backup_at(&backups_dir),
            Some(Local::now().date_naive()),
            "should have created a fresh backup for today"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn applies_retention_after_backing_up() {
        let dir = temp_dir("retention");
        let db_path = dir.join("a.sqlite3");
        db::open_and_migrate(&db_path).unwrap();
        let profiles = vec![profile(1, db_path)];
        let backups_dir = backup::backups_dir(&dir, 1);
        std::fs::create_dir_all(&backups_dir).unwrap();

        for offset in 1..=5 {
            let date = Local::now().date_naive() - ChronoDuration::days(offset);
            std::fs::write(
                backups_dir.join(format!("flashpad-backup-{date}.db")),
                b"old",
            )
            .unwrap();
        }

        run_check_for(&profiles, &dir, 3);

        assert_eq!(
            backup::list_backup_files(&backups_dir).len(),
            3,
            "today's new backup plus the 2 most recent old ones should remain"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn skips_an_unreachable_profile_without_affecting_others() {
        let dir = temp_dir("one-broken");
        let good_path = dir.join("good.sqlite3");
        db::open_and_migrate(&good_path).unwrap();

        let blocked_parent = dir.join("blocked");
        std::fs::write(&blocked_parent, b"not a dir").unwrap();
        let broken_path = blocked_parent.join("bad.sqlite3");

        let profiles = vec![profile(1, good_path), profile(2, broken_path)];

        run_check_for(&profiles, &dir, 7);

        assert_eq!(backup::list_backup_files(&backup::backups_dir(&dir, 1)).len(), 1);
        assert_eq!(backup::list_backup_files(&backup::backups_dir(&dir, 2)).len(), 0);

        let _ = std::fs::remove_dir_all(&dir);
    }
}

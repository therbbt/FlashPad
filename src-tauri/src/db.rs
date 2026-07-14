use rusqlite::{params, Connection, OpenFlags};
use std::path::Path;
use std::sync::Mutex;

const SCHEMA_SQL: &str = "CREATE TABLE IF NOT EXISTS folders (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        parent_id INTEGER REFERENCES folders(id) ON DELETE CASCADE,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS notes (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        title TEXT NOT NULL DEFAULT 'Untitled',
        content TEXT NOT NULL DEFAULT '',
        folder_id INTEGER REFERENCES folders(id) ON DELETE CASCADE,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        is_markdown INTEGER NOT NULL DEFAULT 0,
        is_locked INTEGER NOT NULL DEFAULT 0,
        sort_order INTEGER NOT NULL DEFAULT 0
    );";

/// Holds the live connection to whichever database is currently active.
/// `None` means the configured path is unreachable/unavailable (e.g. an
/// unmounted sync folder) - see `activate()` below - rather than a
/// placeholder empty database ever being silently created in its place.
pub struct DbState(pub Mutex<Option<Connection>>);

/// Explains *why* `DbState` currently holds `None`, kept separate from
/// `DbState` itself so `notes.rs` commands only ever need the generic
/// "no database available" error, while `get_app_state` can surface the
/// specific reason (e.g. "path does not exist") for the startup error UI.
pub enum DbStatus {
    Ready,
    Unavailable(String),
}

pub struct DbStatusState(pub Mutex<DbStatus>);

/// Opens `path` and reports the outcome as both a (possibly absent) live
/// connection and a status describing why - so callers never reconstruct a
/// `DbStatus` from a `Result` themselves. Used identically by initial
/// startup, `switch_database`, and after registering a new profile.
pub fn activate(path: &Path) -> (Option<Connection>, DbStatus) {
    match open_and_migrate(path) {
        Ok(conn) => (Some(conn), DbStatus::Ready),
        Err(err) => (None, DbStatus::Unavailable(err)),
    }
}

/// Creates the schema if missing and runs the full migration chain against
/// an already-open connection. Factored out of `open_and_migrate` so it can
/// also be run after an in-place SQLite restore (import), where the
/// connection was already open before its backing file content changed -
/// there's no new path to open, just the existing connection to bring
/// forward, which matters since an imported file may predate a migration.
pub fn ensure_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(SCHEMA_SQL).map_err(|e| e.to_string())?;

    migrate_legacy_folder_column(conn);
    migrate_folders_into_notes(conn);
    migrate_add_markdown_column(conn);
    migrate_add_locked_column(conn);
    migrate_add_sort_order_column(conn);

    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Opens (creating if needed) and migrates the database at `db_path`.
/// Fallible by design: callers include a background scheduler thread and a
/// "switch database" command, neither of which should be able to crash the
/// whole app just because one configured path is temporarily unreachable.
pub fn open_and_migrate(db_path: &Path) -> Result<Connection, String> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    ensure_schema(&conn)?;
    Ok(conn)
}

/// Cheap, deliberately lenient check that `path` looks like a genuine
/// FlashPad database before it's trusted with an import/relocation/restore:
/// valid SQLite file, passes `quick_check`, and has a `notes` table. Does
/// NOT check the exact current column list, so a legitimate export from an
/// older FlashPad version isn't rejected - `open_and_migrate`'s existing
/// migration chain brings any recognized older shape forward after the file
/// is actually opened for real.
pub fn validate_flashpad_file(path: &Path) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if bytes.len() < 16 || &bytes[0..16] != b"SQLite format 3\0" {
        return Err("Not a valid SQLite database file".to_string());
    }

    // READ_ONLY alone (no CREATE flag) guarantees this check can never
    // itself bring a new empty database file into existence at `path`.
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| e.to_string())?;

    let check: String = conn
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    if check != "ok" {
        return Err(format!("Database failed integrity check: {check}"));
    }

    if !table_exists(&conn, "notes") {
        return Err("File does not contain a FlashPad database".to_string());
    }

    Ok(())
}

/// Earlier builds stored notes.folder as a free-text label, and `CREATE TABLE
/// IF NOT EXISTS` above is a no-op against that pre-existing table, so an old
/// database won't have folder_id yet. Add it, fold the free-text labels into
/// the new folders table (one top-level folder per distinct label), then
/// drop the legacy column, so existing notes end up in the right place.
fn migrate_legacy_folder_column(conn: &Connection) {
    let has_legacy_column = column_exists(conn, "folder");
    if !has_legacy_column {
        return;
    }

    if !column_exists(conn, "folder_id") {
        conn.execute_batch(
            "ALTER TABLE notes ADD COLUMN folder_id INTEGER REFERENCES folders(id) ON DELETE CASCADE;",
        )
        .expect("failed to add folder_id column");
    }

    conn.execute_batch(
        "BEGIN;
        INSERT INTO folders (name, parent_id, created_at, updated_at)
        SELECT DISTINCT folder, NULL, datetime('now'), datetime('now')
        FROM notes
        WHERE folder IS NOT NULL AND folder <> '' AND folder <> 'Notes';

        UPDATE notes SET folder_id = (
            SELECT id FROM folders WHERE folders.name = notes.folder AND folders.parent_id IS NULL
        ) WHERE folder IS NOT NULL AND folder <> '' AND folder <> 'Notes';

        ALTER TABLE notes DROP COLUMN folder;
        COMMIT;",
    )
    .expect("failed to migrate legacy folder column");
}

fn column_exists(conn: &Connection, column: &str) -> bool {
    conn.prepare("SELECT 1 FROM pragma_table_info('notes') WHERE name = ?1")
        .and_then(|mut stmt| stmt.exists(params![column]))
        .unwrap_or(false)
}

fn now_iso() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

fn table_exists(conn: &Connection, table: &str) -> bool {
    conn.prepare("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1")
        .and_then(|mut stmt| stmt.exists(params![table]))
        .unwrap_or(false)
}

/// Folders used to be a separate entity from notes. Now any note can hold
/// child notes directly (parent_id references notes.id), so a folder is just
/// a note with children - fold the old folders table into notes and rewire
/// notes.folder_id into notes.parent_id, preserving hierarchy.
fn migrate_folders_into_notes(conn: &Connection) {
    if !table_exists(conn, "folders") {
        return;
    }

    if !column_exists(conn, "parent_id") {
        conn.execute_batch(
            "ALTER TABLE notes ADD COLUMN parent_id INTEGER REFERENCES notes(id) ON DELETE CASCADE;",
        )
        .expect("failed to add parent_id column");
    }

    struct FolderRow {
        id: i64,
        name: String,
        parent_id: Option<i64>,
    }

    let mut remaining: Vec<FolderRow> = {
        let mut stmt = conn
            .prepare("SELECT id, name, parent_id FROM folders")
            .expect("failed to read folders");
        stmt.query_map([], |row| {
            Ok(FolderRow {
                id: row.get(0)?,
                name: row.get(1)?,
                parent_id: row.get(2)?,
            })
        })
        .expect("failed to read folders")
        .filter_map(Result::ok)
        .collect()
    };

    let now = now_iso();
    let mut id_map: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();

    // Insert folders as notes with parents before children, regardless of
    // the order they came back in (handles arbitrary nesting depth).
    while !remaining.is_empty() {
        let mut progressed = false;
        remaining.retain(|folder| {
            let ready = match folder.parent_id {
                None => true,
                Some(pid) => id_map.contains_key(&pid),
            };
            if !ready {
                return true;
            }
            let new_parent_id = folder.parent_id.map(|pid| id_map[&pid]);
            conn.execute(
                "INSERT INTO notes (title, content, parent_id, created_at, updated_at) VALUES (?1, '', ?2, ?3, ?3)",
                params![folder.name, new_parent_id, now],
            )
            .expect("failed to migrate folder into notes");
            id_map.insert(folder.id, conn.last_insert_rowid());
            progressed = true;
            false
        });
        if !progressed {
            break;
        }
    }

    // Guarded independently of the table_exists(folders) check above: if a
    // previous run of this migration got interrupted after dropping
    // folder_id but before dropping the (by then empty) folders table, the
    // column will already be gone on the next launch.
    if column_exists(conn, "folder_id") {
        for (old_folder_id, new_note_id) in &id_map {
            conn.execute(
                "UPDATE notes SET parent_id = ?1 WHERE folder_id = ?2",
                params![new_note_id, old_folder_id],
            )
            .expect("failed to rewire notes.parent_id");
        }

        conn.execute_batch("ALTER TABLE notes DROP COLUMN folder_id;")
            .expect("failed to drop notes.folder_id");
    }

    conn.execute_batch("DROP TABLE IF EXISTS folders;")
        .expect("failed to drop folders table");
}

/// Adds the per-note markdown toggle to existing databases created before it
/// existed. Fresh installs already get the column via CREATE TABLE above.
fn migrate_add_markdown_column(conn: &Connection) {
    if column_exists(conn, "is_markdown") {
        return;
    }
    conn.execute_batch("ALTER TABLE notes ADD COLUMN is_markdown INTEGER NOT NULL DEFAULT 0;")
        .expect("failed to add is_markdown column");
}

/// Adds the per-note lock toggle to existing databases created before it
/// existed. Fresh installs already get the column via CREATE TABLE above.
fn migrate_add_locked_column(conn: &Connection) {
    if column_exists(conn, "is_locked") {
        return;
    }
    conn.execute_batch("ALTER TABLE notes ADD COLUMN is_locked INTEGER NOT NULL DEFAULT 0;")
        .expect("failed to add is_locked column");
}

/// Adds the manual tree-position column to existing databases created
/// before it existed. Fresh installs already get the column via CREATE
/// TABLE above. Backfills each parent group (including the root, where
/// parent_id IS NULL) with sequential values in today's effective display
/// order (oldest-first by created_at), so existing installs see no visual
/// change immediately after upgrading - only a later drag-and-drop
/// reorder deviates from that order afterwards.
fn migrate_add_sort_order_column(conn: &Connection) {
    if column_exists(conn, "sort_order") {
        return;
    }
    conn.execute_batch("ALTER TABLE notes ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;")
        .expect("failed to add sort_order column");

    let parent_groups: Vec<Option<i64>> = {
        let mut stmt = conn
            .prepare("SELECT DISTINCT parent_id FROM notes")
            .expect("failed to read parent groups");
        stmt.query_map([], |row| row.get::<_, Option<i64>>(0))
            .expect("failed to read parent groups")
            .filter_map(Result::ok)
            .collect()
    };

    for parent_id in parent_groups {
        let ids: Vec<i64> = {
            let mut stmt = conn
                .prepare("SELECT id FROM notes WHERE parent_id IS ?1 ORDER BY created_at, id")
                .expect("failed to read sibling group");
            stmt.query_map(params![parent_id], |row| row.get::<_, i64>(0))
                .expect("failed to read sibling group")
                .filter_map(Result::ok)
                .collect()
        };
        for (index, id) in ids.into_iter().enumerate() {
            conn.execute(
                "UPDATE notes SET sort_order = ?1 WHERE id = ?2",
                params![index as i64, id],
            )
            .expect("failed to backfill sort_order");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("flashpad-db-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(format!("{name}.sqlite3"))
    }

    #[test]
    fn validate_flashpad_file_accepts_a_real_database() {
        let path = temp_path("valid");
        let _ = std::fs::remove_file(&path);
        open_and_migrate(&path).unwrap();

        assert!(validate_flashpad_file(&path).is_ok());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn validate_flashpad_file_rejects_a_non_sqlite_file() {
        let path = temp_path("garbage");
        std::fs::write(&path, b"definitely not a sqlite file").unwrap();

        let result = validate_flashpad_file(&path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Not a valid SQLite database"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn validate_flashpad_file_rejects_a_sqlite_file_without_a_notes_table() {
        let path = temp_path("wrong-schema");
        let _ = std::fs::remove_file(&path);
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE something_else (id INTEGER PRIMARY KEY);")
            .unwrap();
        drop(conn);

        let result = validate_flashpad_file(&path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("FlashPad database"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn validate_flashpad_file_never_creates_a_file_at_a_missing_path() {
        let path = temp_path("does-not-exist");
        let _ = std::fs::remove_file(&path);

        assert!(validate_flashpad_file(&path).is_err());
        assert!(!path.exists(), "validation must never bring a file into existence");
    }

    #[test]
    fn ensure_schema_is_reusable_against_an_already_open_connection() {
        // Mirrors what import_database does: restore changes an existing
        // Connection's backing content, then ensure_schema is re-run against
        // that same live connection (no new path to open).
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE notes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL DEFAULT 'Untitled',
                content TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                parent_id INTEGER REFERENCES notes(id) ON DELETE CASCADE
            );
            INSERT INTO notes (title, content, created_at, updated_at)
            VALUES ('Imported from an older version', '', '2026-01-01T00:00:00', '2026-01-01T00:00:00');",
        )
        .unwrap();

        assert!(!column_exists(&conn, "is_markdown"));
        ensure_schema(&conn).unwrap();
        assert!(column_exists(&conn, "is_markdown"));
        assert!(column_exists(&conn, "sort_order"));

        let note_count: i64 = conn.query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0)).unwrap();
        assert_eq!(note_count, 1, "the imported note must survive being brought forward");
    }

    #[test]
    fn migrates_legacy_folder_text_column_into_folders_table() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE notes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL DEFAULT 'Untitled',
                content TEXT NOT NULL DEFAULT '',
                folder TEXT NOT NULL DEFAULT 'Notes',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            INSERT INTO notes (title, content, folder, created_at, updated_at)
            VALUES ('Root note', '', 'Notes', '2026-01-01T00:00:00', '2026-01-01T00:00:00');
            INSERT INTO notes (title, content, folder, created_at, updated_at)
            VALUES ('Work note', '', 'Work', '2026-01-01T00:00:00', '2026-01-01T00:00:00');
            INSERT INTO notes (title, content, folder, created_at, updated_at)
            VALUES ('Another work note', '', 'Work', '2026-01-01T00:00:00', '2026-01-01T00:00:00');",
        )
        .unwrap();

        // Mirror init()'s ordering: CREATE TABLE IF NOT EXISTS folders (new),
        // notes already exists with the legacy schema so it's untouched, then migrate.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS folders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                parent_id INTEGER REFERENCES folders(id) ON DELETE CASCADE,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );",
        )
        .unwrap();

        migrate_legacy_folder_column(&conn);

        assert!(!column_exists(&conn, "folder"));
        assert!(column_exists(&conn, "folder_id"));

        let folder_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM folders", [], |r| r.get(0))
            .unwrap();
        assert_eq!(folder_count, 1, "only 'Work' should become a folder; 'Notes' is the implicit root");

        let root_note_folder: Option<i64> = conn
            .query_row(
                "SELECT folder_id FROM notes WHERE title = 'Root note'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(root_note_folder, None);

        let work_notes: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM notes n JOIN folders f ON n.folder_id = f.id WHERE f.name = 'Work'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(work_notes, 2);
    }

    #[test]
    fn init_creates_fresh_schema_without_migration() {
        let dir = std::env::temp_dir().join(format!("flashpad-test-{}", std::process::id()));
        let db_path = dir.join("test.sqlite3");
        let conn = open_and_migrate(&db_path).unwrap();

        assert!(column_exists(&conn, "parent_id"));
        assert!(!column_exists(&conn, "folder_id"));
        assert!(!column_exists(&conn, "folder"));
        assert!(!table_exists(&conn, "folders"));
        assert!(column_exists(&conn, "is_markdown"));
        assert!(column_exists(&conn, "is_locked"));
        assert!(column_exists(&conn, "sort_order"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn migrate_add_markdown_column_adds_it_to_existing_databases() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE notes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL DEFAULT 'Untitled',
                content TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                parent_id INTEGER REFERENCES notes(id) ON DELETE CASCADE
            );
            INSERT INTO notes (title, content, created_at, updated_at, parent_id)
            VALUES ('Existing note', '', '2026-01-01T00:00:00', '2026-01-01T00:00:00', NULL);",
        )
        .unwrap();

        assert!(!column_exists(&conn, "is_markdown"));
        migrate_add_markdown_column(&conn);
        assert!(column_exists(&conn, "is_markdown"));

        let is_markdown: bool = conn
            .query_row("SELECT is_markdown FROM notes WHERE title = 'Existing note'", [], |r| r.get(0))
            .unwrap();
        assert!(!is_markdown, "existing notes should default to plain text");

        // Safe to rerun.
        migrate_add_markdown_column(&conn);
    }

    #[test]
    fn migrate_add_locked_column_adds_it_to_existing_databases() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE notes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL DEFAULT 'Untitled',
                content TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                parent_id INTEGER REFERENCES notes(id) ON DELETE CASCADE,
                is_markdown INTEGER NOT NULL DEFAULT 0
            );
            INSERT INTO notes (title, content, created_at, updated_at, parent_id, is_markdown)
            VALUES ('Existing note', '', '2026-01-01T00:00:00', '2026-01-01T00:00:00', NULL, 0);",
        )
        .unwrap();

        assert!(!column_exists(&conn, "is_locked"));
        migrate_add_locked_column(&conn);
        assert!(column_exists(&conn, "is_locked"));

        let is_locked: bool = conn
            .query_row("SELECT is_locked FROM notes WHERE title = 'Existing note'", [], |r| r.get(0))
            .unwrap();
        assert!(!is_locked, "existing notes should default to unlocked");

        // Safe to rerun.
        migrate_add_locked_column(&conn);
    }

    #[test]
    fn migrate_add_sort_order_column_backfills_existing_order() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE notes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL DEFAULT 'Untitled',
                content TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                parent_id INTEGER REFERENCES notes(id) ON DELETE CASCADE,
                is_markdown INTEGER NOT NULL DEFAULT 0,
                is_locked INTEGER NOT NULL DEFAULT 0
            );
            INSERT INTO notes (id, title, content, created_at, updated_at, parent_id)
            VALUES (1, 'Root A', '', '2026-01-01T00:00:00', '2026-01-01T00:00:00', NULL);
            INSERT INTO notes (id, title, content, created_at, updated_at, parent_id)
            VALUES (2, 'Root B', '', '2026-01-02T00:00:00', '2026-01-02T00:00:00', NULL);
            INSERT INTO notes (id, title, content, created_at, updated_at, parent_id)
            VALUES (3, 'Child of A, older', '', '2026-01-01T01:00:00', '2026-01-01T01:00:00', 1);
            INSERT INTO notes (id, title, content, created_at, updated_at, parent_id)
            VALUES (4, 'Child of A, newer', '', '2026-01-01T02:00:00', '2026-01-01T02:00:00', 1);",
        )
        .unwrap();

        assert!(!column_exists(&conn, "sort_order"));
        migrate_add_sort_order_column(&conn);
        assert!(column_exists(&conn, "sort_order"));

        let root_a_order: i64 = conn
            .query_row("SELECT sort_order FROM notes WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        let root_b_order: i64 = conn
            .query_row("SELECT sort_order FROM notes WHERE id = 2", [], |r| r.get(0))
            .unwrap();
        assert!(root_a_order < root_b_order, "Root A was created first, so should sort before Root B");

        let child_older_order: i64 = conn
            .query_row("SELECT sort_order FROM notes WHERE id = 3", [], |r| r.get(0))
            .unwrap();
        let child_newer_order: i64 = conn
            .query_row("SELECT sort_order FROM notes WHERE id = 4", [], |r| r.get(0))
            .unwrap();
        assert!(child_older_order < child_newer_order);

        // Safe to rerun.
        migrate_add_sort_order_column(&conn);
    }

    #[test]
    fn migrates_folders_table_into_notes_preserving_hierarchy() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE folders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                parent_id INTEGER REFERENCES folders(id) ON DELETE CASCADE,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE notes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL DEFAULT 'Untitled',
                content TEXT NOT NULL DEFAULT '',
                folder_id INTEGER REFERENCES folders(id) ON DELETE CASCADE,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            INSERT INTO folders (id, name, parent_id, created_at, updated_at)
            VALUES (1, 'Work', NULL, '2026-01-01T00:00:00', '2026-01-01T00:00:00');
            INSERT INTO folders (id, name, parent_id, created_at, updated_at)
            VALUES (2, 'Projects', 1, '2026-01-01T00:00:00', '2026-01-01T00:00:00');
            INSERT INTO notes (title, content, folder_id, created_at, updated_at)
            VALUES ('Root note', '', NULL, '2026-01-01T00:00:00', '2026-01-01T00:00:00');
            INSERT INTO notes (title, content, folder_id, created_at, updated_at)
            VALUES ('Nested note', '', 2, '2026-01-01T00:00:00', '2026-01-01T00:00:00');",
        )
        .unwrap();

        migrate_folders_into_notes(&conn);

        assert!(column_exists(&conn, "parent_id"));
        assert!(!column_exists(&conn, "folder_id"));
        assert!(!table_exists(&conn, "folders"));

        let root_note_parent: Option<i64> = conn
            .query_row("SELECT parent_id FROM notes WHERE title = 'Root note'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(root_note_parent, None);

        // 'Nested note' lived in 'Projects', which lived in 'Work' - both should
        // now be notes themselves, with the hierarchy preserved via parent_id.
        let nested_note_parent_name: String = conn
            .query_row(
                "SELECT p.title FROM notes n JOIN notes p ON n.parent_id = p.id WHERE n.title = 'Nested note'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(nested_note_parent_name, "Projects");

        let projects_parent_name: String = conn
            .query_row(
                "SELECT p.title FROM notes n JOIN notes p ON n.parent_id = p.id WHERE n.title = 'Projects'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(projects_parent_name, "Work");

        let work_parent: Option<i64> = conn
            .query_row("SELECT parent_id FROM notes WHERE title = 'Work'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(work_parent, None);
    }

    #[test]
    fn migrate_folders_into_notes_is_safe_to_rerun_after_a_partial_run() {
        // Simulates an interrupted prior run: parent_id already added and
        // populated, folder_id already dropped, but the (now empty) folders
        // table wasn't dropped yet. Rerunning the migration must not panic.
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE folders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                parent_id INTEGER REFERENCES folders(id) ON DELETE CASCADE,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE notes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL DEFAULT 'Untitled',
                content TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                parent_id INTEGER REFERENCES notes(id) ON DELETE CASCADE
            );
            INSERT INTO notes (title, content, created_at, updated_at, parent_id)
            VALUES ('Existing note', '', '2026-01-01T00:00:00', '2026-01-01T00:00:00', NULL);",
        )
        .unwrap();

        migrate_folders_into_notes(&conn);

        assert!(!table_exists(&conn, "folders"));
        assert!(column_exists(&conn, "parent_id"));
        let note_count: i64 = conn.query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0)).unwrap();
        assert_eq!(note_count, 1, "the pre-existing note should be untouched");
    }
}

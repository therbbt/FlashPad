use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct DbState(pub Mutex<Connection>);

pub fn init(db_path: PathBuf) -> Connection {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).expect("failed to create app data dir");
    }
    let conn = Connection::open(db_path).expect("failed to open database");
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS folders (
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
            updated_at TEXT NOT NULL
        );",
    )
    .expect("failed to create schema");

    migrate_legacy_folder_column(&conn);

    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .expect("failed to enable foreign keys");

    conn
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
        .and_then(|mut stmt| stmt.exists(rusqlite::params![column]))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let conn = init(db_path);

        assert!(column_exists(&conn, "folder_id"));
        assert!(!column_exists(&conn, "folder"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}

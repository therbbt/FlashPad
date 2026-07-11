use crate::db::DbState;
use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: i64,
    pub title: String,
    pub content: String,
    pub parent_id: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteInput {
    pub title: Option<String>,
    pub content: Option<String>,
    pub parent_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct NoteUpdate {
    pub id: i64,
    pub title: Option<String>,
    pub content: Option<String>,
}

const SELECT_COLUMNS: &str = "id, title, content, parent_id, created_at, updated_at";

fn row_to_note(row: &Row) -> rusqlite::Result<Note> {
    Ok(Note {
        id: row.get(0)?,
        title: row.get(1)?,
        content: row.get(2)?,
        parent_id: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn now_iso() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

fn find_note(conn: &rusqlite::Connection, id: i64) -> Result<Note, String> {
    conn.query_row(
        &format!("SELECT {SELECT_COLUMNS} FROM notes WHERE id = ?1"),
        params![id],
        row_to_note,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_notes(db: State<DbState>) -> Result<Vec<Note>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM notes ORDER BY updated_at DESC"
        ))
        .map_err(|e| e.to_string())?;
    let notes = stmt
        .query_map([], row_to_note)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();
    Ok(notes)
}

#[tauri::command]
pub fn create_note(db: State<DbState>, note: NoteInput) -> Result<Note, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let now = now_iso();
    let title = note.title.unwrap_or_else(|| "Untitled".to_string());
    let content = note.content.unwrap_or_default();

    conn.execute(
        "INSERT INTO notes (title, content, parent_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
        params![title, content, note.parent_id, now],
    )
    .map_err(|e| e.to_string())?;

    find_note(&conn, conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_note(db: State<DbState>, note: NoteUpdate) -> Result<Note, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let existing = find_note(&conn, note.id)?;

    let title = note.title.unwrap_or(existing.title);
    let content = note.content.unwrap_or(existing.content);
    let now = now_iso();

    conn.execute(
        "UPDATE notes SET title = ?1, content = ?2, updated_at = ?3 WHERE id = ?4",
        params![title, content, now, note.id],
    )
    .map_err(|e| e.to_string())?;

    find_note(&conn, note.id)
}

#[tauri::command]
pub fn delete_note(db: State<DbState>, id: i64) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM notes WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn move_note(db: State<DbState>, id: i64, parent_id: Option<i64>) -> Result<Note, String> {
    if parent_id == Some(id) {
        return Err("A note cannot be moved into itself".into());
    }

    let conn = db.0.lock().map_err(|e| e.to_string())?;

    if let Some(target) = parent_id {
        let mut cursor = Some(target);
        while let Some(current) = cursor {
            if current == id {
                return Err("Cannot move a note into one of its own subnotes".into());
            }
            cursor = conn
                .query_row(
                    "SELECT parent_id FROM notes WHERE id = ?1",
                    params![current],
                    |row| row.get::<_, Option<i64>>(0),
                )
                .map_err(|e| e.to_string())?;
        }
    }

    let now = now_iso();
    conn.execute(
        "UPDATE notes SET parent_id = ?1, updated_at = ?2 WHERE id = ?3",
        params![parent_id, now, id],
    )
    .map_err(|e| e.to_string())?;

    find_note(&conn, id)
}

#[tauri::command]
pub fn duplicate_note(db: State<DbState>, id: i64) -> Result<Note, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let source = find_note(&conn, id)?;
    let now = now_iso();
    let title = format!("{} (copy)", source.title);

    conn.execute(
        "INSERT INTO notes (title, content, parent_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
        params![title, source.content, source.parent_id, now],
    )
    .map_err(|e| e.to_string())?;

    find_note(&conn, conn.last_insert_rowid())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_input_deserializes_camel_case_parent_id() {
        let input: NoteInput = serde_json::from_str(r#"{"title":"Hi","content":"","parentId":42}"#).unwrap();
        assert_eq!(input.parent_id, Some(42));
    }

    #[test]
    fn note_input_defaults_parent_id_to_root_when_omitted() {
        let input: NoteInput = serde_json::from_str(r#"{"title":"Hi","content":""}"#).unwrap();
        assert_eq!(input.parent_id, None);
    }
}

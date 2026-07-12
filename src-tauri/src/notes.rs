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
    pub is_markdown: bool,
    pub is_locked: bool,
    pub sort_order: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteInput {
    pub title: Option<String>,
    pub content: Option<String>,
    pub parent_id: Option<i64>,
    pub is_markdown: Option<bool>,
    pub is_locked: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteUpdate {
    pub id: i64,
    pub title: Option<String>,
    pub content: Option<String>,
    pub is_markdown: Option<bool>,
    pub is_locked: Option<bool>,
}

const SELECT_COLUMNS: &str = "id, title, content, parent_id, created_at, updated_at, is_markdown, is_locked, sort_order";

fn row_to_note(row: &Row) -> rusqlite::Result<Note> {
    Ok(Note {
        id: row.get(0)?,
        title: row.get(1)?,
        content: row.get(2)?,
        parent_id: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
        is_markdown: row.get(6)?,
        is_locked: row.get(7)?,
        sort_order: row.get(8)?,
    })
}

fn next_sort_order(conn: &rusqlite::Connection, parent_id: Option<i64>) -> Result<i64, String> {
    conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM notes WHERE parent_id IS ?1",
        params![parent_id],
        |row| row.get(0),
    )
    .map_err(|e| e.to_string())
}

/// Shared by `move_note` and `reorder_note`: walks the parent chain from
/// `parent_id` back up to the root, rejecting a move that would place `id`
/// inside itself or one of its own descendants.
fn check_no_cycle(conn: &rusqlite::Connection, id: i64, parent_id: Option<i64>) -> Result<(), String> {
    if parent_id == Some(id) {
        return Err("A note cannot be moved into itself".into());
    }
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
    Ok(())
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
    let is_markdown = note.is_markdown.unwrap_or(false);
    let is_locked = note.is_locked.unwrap_or(false);
    let sort_order = next_sort_order(&conn, note.parent_id)?;

    conn.execute(
        "INSERT INTO notes (title, content, parent_id, created_at, updated_at, is_markdown, is_locked, sort_order) VALUES (?1, ?2, ?3, ?4, ?4, ?5, ?6, ?7)",
        params![title, content, note.parent_id, now, is_markdown, is_locked, sort_order],
    )
    .map_err(|e| e.to_string())?;

    find_note(&conn, conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_note(db: State<DbState>, note: NoteUpdate) -> Result<Note, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let existing = find_note(&conn, note.id)?;

    let is_locked = note.is_locked.unwrap_or(existing.is_locked);

    // Defense-in-depth: the primary enforcement is the read-only editor in
    // the UI, but reject here too in case a stray autosave races the lock.
    // Unlocking-and-editing in the same call (e.g. from a future "unlock and
    // edit" action) is still allowed.
    if existing.is_locked
        && is_locked
        && (note.title.as_ref().is_some_and(|t| *t != existing.title)
            || note.content.as_ref().is_some_and(|c| *c != existing.content))
    {
        return Err("Note is locked".into());
    }

    let title = note.title.unwrap_or(existing.title);
    let content = note.content.unwrap_or(existing.content);
    let is_markdown = note.is_markdown.unwrap_or(existing.is_markdown);
    let now = now_iso();

    conn.execute(
        "UPDATE notes SET title = ?1, content = ?2, updated_at = ?3, is_markdown = ?4, is_locked = ?5 WHERE id = ?6",
        params![title, content, now, is_markdown, is_locked, note.id],
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
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    check_no_cycle(&conn, id, parent_id)?;

    let now = now_iso();
    let sort_order = next_sort_order(&conn, parent_id)?;
    conn.execute(
        "UPDATE notes SET parent_id = ?1, updated_at = ?2, sort_order = ?3 WHERE id = ?4",
        params![parent_id, now, sort_order, id],
    )
    .map_err(|e| e.to_string())?;

    find_note(&conn, id)
}

/// Repositions `id` among the children of `parent_id`, immediately before
/// `before_id` (or at the end if `before_id` is `None` or not currently a
/// child of `parent_id`). Renumbers the whole resulting sibling list so
/// ordering stays a plain dense sequence, avoiding fractional-index
/// precision issues. Used by the sidebar's drag-and-drop tree reordering.
#[tauri::command]
pub fn reorder_note(db: State<DbState>, id: i64, parent_id: Option<i64>, before_id: Option<i64>) -> Result<Note, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    check_no_cycle(&conn, id, parent_id)?;

    let mut stmt = conn
        .prepare("SELECT id FROM notes WHERE parent_id IS ?1 AND id != ?2 ORDER BY sort_order, id")
        .map_err(|e| e.to_string())?;
    let mut siblings: Vec<i64> = stmt
        .query_map(params![parent_id, id], |row| row.get::<_, i64>(0))
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();
    drop(stmt);

    let insert_at = before_id
        .and_then(|b| siblings.iter().position(|&sid| sid == b))
        .unwrap_or(siblings.len());
    siblings.insert(insert_at, id);

    let now = now_iso();
    for (index, sibling_id) in siblings.iter().enumerate() {
        if *sibling_id == id {
            // Only the dragged note's own parent/updated_at actually changes;
            // untouched siblings just get a renumbered sort_order.
            conn.execute(
                "UPDATE notes SET parent_id = ?1, sort_order = ?2, updated_at = ?3 WHERE id = ?4",
                params![parent_id, index as i64, now, sibling_id],
            )
        } else {
            conn.execute(
                "UPDATE notes SET sort_order = ?1 WHERE id = ?2",
                params![index as i64, sibling_id],
            )
        }
        .map_err(|e| e.to_string())?;
    }

    find_note(&conn, id)
}

#[tauri::command]
pub fn duplicate_note(db: State<DbState>, id: i64) -> Result<Note, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let source = find_note(&conn, id)?;
    let now = now_iso();
    let title = format!("{} (copy)", source.title);
    let sort_order = next_sort_order(&conn, source.parent_id)?;

    // A duplicate is never locked, even if the source is - it's a fresh copy
    // the user will likely want to edit further.
    conn.execute(
        "INSERT INTO notes (title, content, parent_id, created_at, updated_at, is_markdown, is_locked, sort_order) VALUES (?1, ?2, ?3, ?4, ?4, ?5, 0, ?6)",
        params![title, source.content, source.parent_id, now, source.is_markdown, sort_order],
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

    #[test]
    fn note_input_deserializes_camel_case_is_markdown() {
        let input: NoteInput = serde_json::from_str(r#"{"title":"Hi","content":"","isMarkdown":true}"#).unwrap();
        assert_eq!(input.is_markdown, Some(true));
    }

    #[test]
    fn note_update_deserializes_camel_case_is_markdown() {
        let input: NoteUpdate = serde_json::from_str(r#"{"id":1,"isMarkdown":true}"#).unwrap();
        assert_eq!(input.is_markdown, Some(true));
    }

    #[test]
    fn note_input_deserializes_camel_case_is_locked() {
        let input: NoteInput = serde_json::from_str(r#"{"title":"Hi","content":"","isLocked":true}"#).unwrap();
        assert_eq!(input.is_locked, Some(true));
    }

    #[test]
    fn note_update_deserializes_camel_case_is_locked() {
        let input: NoteUpdate = serde_json::from_str(r#"{"id":1,"isLocked":true}"#).unwrap();
        assert_eq!(input.is_locked, Some(true));
    }
}

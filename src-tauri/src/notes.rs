use crate::db::DbState;
use regex::Regex;
use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use std::sync::LazyLock;
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
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No database is currently available")?;
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
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No database is currently available")?;
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
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No database is currently available")?;
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
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No database is currently available")?;
    conn.execute("DELETE FROM notes WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn move_note(db: State<DbState>, id: i64, parent_id: Option<i64>) -> Result<Note, String> {
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No database is currently available")?;
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
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No database is currently available")?;
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
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No database is currently available")?;
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

// ---------- FlashNote folder import ----------
//
// FlashNote (a different, unrelated notes app) exports a note tree as plain
// folders/files: a folder is a subnote, and a same-named ".txt" sibling next
// to that folder holds the *folder's own* note content (so "Foo/" plus
// "Foo.txt" is a subnote titled "Foo" with both its own text and children).
// A ".txt" file with no matching folder is just a leaf note.
//
// Each ".txt" file is not plain text, though - it's FlashNote's own autosave
// log: a chronological sequence of timestamped snapshots appended to the
// same file, e.g.:
//
//   <content>
//   =-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=--=
//   <content>
//   =-=-=-=-=- 7/10/2026 1:47:56 PM =-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=
//   <content>
//   ...
//
// Reverse-engineered from sample exports (no official spec), the rule that
// fits every sample: split on divider lines, and the note's *current*
// content is whatever follows the very last divider. The one wrinkle is the
// undated "=-=-...--=" divider variant, which shows up as a trailing
// end-of-log marker with nothing after it - when that's the last divider
// and the trailing segment is blank, the real content is the last
// *non-blank* segment before it instead. See the unit tests below for the
// exact sample data this was derived from.

/// Matches both FlashNote divider styles: a plain run of `=`/`-` (the
/// undated "end of log" marker), or the same run bracketing a timestamp
/// (`M/D/YYYY H:MM:SS AM/PM`) for a dated autosave snapshot.
static DIVIDER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[=\-]{5,}(?: \d{1,2}/\d{1,2}/\d{4} \d{1,2}:\d{2}:\d{2} (?:AM|PM) [=\-]{5,})?$").unwrap()
});

fn extract_current_content(raw: &str) -> String {
    let normalized = raw.replace("\r\n", "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();

    // (line index, whether this divider has an embedded timestamp)
    let dividers: Vec<(usize, bool)> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| DIVIDER_RE.is_match(line))
        .map(|(i, line)| (i, line.contains('/')))
        .collect();

    let Some(&(_, last_divider_has_timestamp)) = dividers.last() else {
        return normalized.trim().to_string();
    };

    let mut segments: Vec<String> = Vec::with_capacity(dividers.len() + 1);
    let mut start = 0;
    for &(pos, _) in &dividers {
        segments.push(lines[start..pos].join("\n"));
        start = pos + 1;
    }
    segments.push(lines[start..].join("\n"));

    let last_segment = segments.last().unwrap().trim();
    if !last_segment.is_empty() || last_divider_has_timestamp {
        return last_segment.to_string();
    }

    // Trailing undated divider with nothing after it - walk back to the
    // last actual content instead of importing it as empty.
    segments
        .iter()
        .rev()
        .skip(1)
        .map(|s| s.trim())
        .find(|s| !s.is_empty())
        .unwrap_or("")
        .to_string()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub imported_count: i64,
    // The first note created (in listing order) - not necessarily
    // meaningful as "the" result since there's no single wrapper note for
    // the import, just something reasonable for the frontend to select
    // afterward so the user lands somewhere instead of nowhere. `None` only
    // if the picked folder was completely empty.
    pub first_note_id: Option<i64>,
}

/// Recurses into `dir`, creating one note per subdirectory (paired with its
/// same-named ".txt" sibling for that subnote's own content, if present) and
/// one note per otherwise-unmatched ".txt" file. Runs inside the caller's
/// transaction, so a mid-import failure (e.g. an unreadable file) leaves the
/// database untouched rather than half-imported.
fn import_dir(
    conn: &rusqlite::Connection,
    dir: &Path,
    parent_id: Option<i64>,
    count: &mut i64,
    first_note_id: &mut Option<i64>,
) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("Failed to read {}: {e}", dir.display()))?;

    let mut subdirs: Vec<std::path::PathBuf> = Vec::new();
    let mut txt_files: Vec<std::path::PathBuf> = Vec::new();
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            subdirs.push(path);
        } else if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("txt")) {
            txt_files.push(path);
        }
    }

    let subdir_names: HashSet<String> = subdirs
        .iter()
        .filter_map(|d| d.file_name().and_then(|n| n.to_str()).map(str::to_string))
        .collect();

    for subdir in &subdirs {
        let name = subdir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled")
            .to_string();
        let sibling_txt = subdir.with_extension("txt");
        let content = if sibling_txt.is_file() {
            extract_current_content(&std::fs::read_to_string(&sibling_txt).map_err(|e| e.to_string())?)
        } else {
            String::new()
        };

        let now = now_iso();
        let sort_order = next_sort_order(conn, parent_id)?;
        conn.execute(
            "INSERT INTO notes (title, content, parent_id, created_at, updated_at, is_markdown, is_locked, sort_order) VALUES (?1, ?2, ?3, ?4, ?4, 0, 0, ?5)",
            params![name, content, parent_id, now, sort_order],
        )
        .map_err(|e| e.to_string())?;
        let new_id = conn.last_insert_rowid();
        *count += 1;
        first_note_id.get_or_insert(new_id);

        import_dir(conn, subdir, Some(new_id), count, first_note_id)?;
    }

    for txt_file in &txt_files {
        let stem = txt_file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string();
        // Already imported above as that subdirectory's own content.
        if subdir_names.contains(&stem) {
            continue;
        }

        let content = extract_current_content(&std::fs::read_to_string(txt_file).map_err(|e| e.to_string())?);
        let now = now_iso();
        let sort_order = next_sort_order(conn, parent_id)?;
        conn.execute(
            "INSERT INTO notes (title, content, parent_id, created_at, updated_at, is_markdown, is_locked, sort_order) VALUES (?1, ?2, ?3, ?4, ?4, 0, 0, ?5)",
            params![stem, content, parent_id, now, sort_order],
        )
        .map_err(|e| e.to_string())?;
        first_note_id.get_or_insert(conn.last_insert_rowid());
        *count += 1;
    }

    Ok(())
}

/// The picked folder's own subfolders/files become top-level notes directly
/// (parent_id NULL) - no extra wrapper note representing the picked folder
/// itself, so a re-import (or importing two exports side by side) just adds
/// more top-level notes rather than nesting wrappers inside wrappers.
#[tauri::command]
pub fn import_flashnote_folder(db: State<DbState>, path: String) -> Result<ImportSummary, String> {
    let root_path = Path::new(&path);

    let mut guard = db.0.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_mut().ok_or("No database is currently available")?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    let mut imported_count = 0i64;
    let mut first_note_id = None;
    import_dir(&tx, root_path, None, &mut imported_count, &mut first_note_id)?;

    tx.commit().map_err(|e| e.to_string())?;

    Ok(ImportSummary {
        imported_count,
        first_note_id,
    })
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

    // extract_current_content - fixtures are byte-for-byte copies of a real
    // FlashNote export's .txt files (see the "flashnote like structure"
    // sample folder), covering every shape found there: no divider at all,
    // a trailing timestamped divider with real content after it, and a
    // trailing undated divider with nothing after it.

    #[test]
    fn extract_current_content_returns_whole_file_when_no_divider_present() {
        assert_eq!(extract_current_content("Samtallista"), "Samtallista");
    }

    #[test]
    fn extract_current_content_takes_segment_after_last_timestamped_divider() {
        let raw = "\r\n=-=-=-=-=- 7/18/2026 3:28:11 PM =-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=\r\nsamtal\r\n\r\n\r\n=-=-=-=-=- 7/18/2026 3:28:14 PM =-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=\r\n\r\n\r\nsamtal\r\n\r\n\r\n=-=-=-=-=- 7/18/2026 3:28:17 PM =-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=\r\n\r\nsamtal\r\n\r\n\r\n=-=-=-=-=- 7/18/2026 3:28:19 PM =-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=\r\n\r\nsamtal\r\n\r\n\r\n=-=-=-=-=- 7/18/2026 3:28:22 PM =-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=\r\n\r\nsamtal\r\n\r\n\r\n=-=-=-=-=- 7/18/2026 3:28:25 PM =-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=\r\n\r\nsamtal\r\n\r\n\r\n=-=-=-=-=- 7/18/2026 3:28:27 PM =-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=\r\n\r\n\r\nsamtal\r\n\r\n\r\n=-=-=-=-=- 7/18/2026 3:28:29 PM =-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=\r\n\r\nsamtal";
        assert_eq!(extract_current_content(raw), "samtal");
    }

    #[test]
    fn extract_current_content_takes_segment_after_last_divider_short_history() {
        let raw = "\r\n=-=-=-=-=- 7/18/2026 3:27:44 PM =-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=\r\nSamltal\r\n\r\n\r\n=-=-=-=-=- 7/18/2026 3:27:46 PM =-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=\r\n\r\n\r\nsamtal\r\n\r\n\r\n=-=-=-=-=- 7/18/2026 3:27:51 PM =-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=\r\n\r\n\r\nsamtal";
        assert_eq!(extract_current_content(raw), "samtal");
    }

    #[test]
    fn extract_current_content_is_empty_when_last_segment_after_timestamped_divider_is_blank() {
        // An untouched "New Note" placeholder: the log's final entry (a
        // timestamped divider) has nothing after it - unlike the undated
        // "end of log" divider, a blank segment after a *timestamped* one is
        // taken at face value (the note really was last saved empty).
        let raw = "Write your note here\r\n=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=--=\r\n7/10/2026 2:21:42 PM\r\n=-=-=-=-=- 7/10/2026 2:21:42 PM =-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=\r\n\r\n=-=-=-=-=- 7/10/2026 2:27:48 PM =-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=\r\n";
        assert_eq!(extract_current_content(raw), "");
    }

    #[test]
    fn extract_current_content_falls_back_past_trailing_undated_divider() {
        // Ends on the undated "end of log" divider with nothing after it -
        // the real current content is the last non-blank segment before it.
        let raw = "New line\r\n=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=--=\r\nDate time\r\n\r\n7/10/2026 1:47:53 PM\r\n\r\nNew Line with datetime \r\n=-=-=-=-=- 7/10/2026 1:47:56 PM =-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=\r\n\r\n=-=-=-=-=- 7/10/2026 1:54:55 PM =-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=\r\n\r\n=-=-=-=-=- 7/10/2026 1:54:57 PM =-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=\r\n\r\n=-=-=-=-=- 7/10/2026 1:54:58 PM =-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=\r\n\r\n=-=-=-=-=- 7/10/2026 1:54:59 PM =-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=\r\nnetwst jadaja\r\n\r\n\r\n=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=--=\r\n";
        assert_eq!(extract_current_content(raw), "netwst jadaja");
    }
}

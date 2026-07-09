use crate::db::DbState;
use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderInput {
    pub name: String,
    pub parent_id: Option<i64>,
}

const SELECT_COLUMNS: &str = "id, name, parent_id, created_at, updated_at";

fn row_to_folder(row: &Row) -> rusqlite::Result<Folder> {
    Ok(Folder {
        id: row.get(0)?,
        name: row.get(1)?,
        parent_id: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

fn now_iso() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

#[tauri::command]
pub fn list_folders(db: State<DbState>) -> Result<Vec<Folder>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(&format!("SELECT {SELECT_COLUMNS} FROM folders ORDER BY name COLLATE NOCASE"))
        .map_err(|e| e.to_string())?;
    let folders = stmt
        .query_map([], row_to_folder)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();
    Ok(folders)
}

#[tauri::command]
pub fn create_folder(db: State<DbState>, folder: FolderInput) -> Result<Folder, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let now = now_iso();

    conn.execute(
        "INSERT INTO folders (name, parent_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
        params![folder.name, folder.parent_id, now],
    )
    .map_err(|e| e.to_string())?;

    let id = conn.last_insert_rowid();
    conn.query_row(
        &format!("SELECT {SELECT_COLUMNS} FROM folders WHERE id = ?1"),
        params![id],
        row_to_folder,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rename_folder(db: State<DbState>, id: i64, name: String) -> Result<Folder, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let now = now_iso();

    conn.execute(
        "UPDATE folders SET name = ?1, updated_at = ?2 WHERE id = ?3",
        params![name, now, id],
    )
    .map_err(|e| e.to_string())?;

    conn.query_row(
        &format!("SELECT {SELECT_COLUMNS} FROM folders WHERE id = ?1"),
        params![id],
        row_to_folder,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn move_folder(db: State<DbState>, id: i64, parent_id: Option<i64>) -> Result<Folder, String> {
    if parent_id == Some(id) {
        return Err("A folder cannot be moved into itself".into());
    }

    let conn = db.0.lock().map_err(|e| e.to_string())?;

    if let Some(target) = parent_id {
        let mut cursor = Some(target);
        while let Some(current) = cursor {
            if current == id {
                return Err("Cannot move a folder into one of its own subfolders".into());
            }
            cursor = conn
                .query_row(
                    "SELECT parent_id FROM folders WHERE id = ?1",
                    params![current],
                    |row| row.get::<_, Option<i64>>(0),
                )
                .map_err(|e| e.to_string())?;
        }
    }

    let now = now_iso();
    conn.execute(
        "UPDATE folders SET parent_id = ?1, updated_at = ?2 WHERE id = ?3",
        params![parent_id, now, id],
    )
    .map_err(|e| e.to_string())?;

    conn.query_row(
        &format!("SELECT {SELECT_COLUMNS} FROM folders WHERE id = ?1"),
        params![id],
        row_to_folder,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_folder(db: State<DbState>, id: i64) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM folders WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_input_deserializes_camel_case_parent_id() {
        let input: FolderInput = serde_json::from_str(r#"{"name":"Work","parentId":7}"#).unwrap();
        assert_eq!(input.parent_id, Some(7));
    }

    #[test]
    fn folder_input_defaults_parent_id_to_root_when_omitted() {
        let input: FolderInput = serde_json::from_str(r#"{"name":"Work"}"#).unwrap();
        assert_eq!(input.parent_id, None);
    }
}

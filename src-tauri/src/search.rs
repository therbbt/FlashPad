use crate::db;
use crate::notes::{self, Note};
use crate::profiles::ConfigState;
use serde::Serialize;
use tauri::State;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossDatabaseNote {
    pub database_id: u32,
    pub database_name: String,
    #[serde(flatten)]
    pub note: Note,
}

/// Fetches every note from every registered database EXCEPT the active one
/// - the active database's notes are already loaded into the frontend via
/// `list_notes`, so returning them again here would just duplicate data.
/// Opens each other database the same way `switch_database` does
/// (`db::open_and_migrate`, which creates the schema if missing and runs
/// the full migration chain) - safe and idempotent, but does mean a
/// database that hasn't been opened in a while gets silently brought up to
/// the current schema version the first time this runs. A database that
/// fails to open (e.g. an unplugged external drive) is skipped rather than
/// failing the whole search.
#[tauri::command]
pub fn list_notes_from_other_databases(
    config: State<ConfigState>,
) -> Result<Vec<CrossDatabaseNote>, String> {
    let config = config.0.lock().map_err(|e| e.to_string())?;
    let mut results = Vec::new();

    for profile in &config.databases {
        if profile.id == config.active_database_id {
            continue;
        }
        let conn = match db::open_and_migrate(&profile.path) {
            Ok(conn) => conn,
            Err(err) => {
                eprintln!(
                    "[flashpad] skipping database '{}' in cross-database search: {err}",
                    profile.name
                );
                continue;
            }
        };
        let notes = notes::fetch_all(&conn)?;
        results.extend(notes.into_iter().map(|note| CrossDatabaseNote {
            database_id: profile.id,
            database_name: profile.name.clone(),
            note,
        }));
    }

    Ok(results)
}

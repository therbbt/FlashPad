import { derived, get, writable } from 'svelte/store';
import { DatabaseService, type AppState, type DatabaseProfile, type CrossDatabaseNote } from '../services/databaseService';

const databaseService = new DatabaseService();

export const databases = writable<DatabaseProfile[]>([]);
export const activeDatabaseId = writable<number | null>(null);
// Set when the configured active database is unreachable (e.g. an
// unmounted sync folder) - callers replace the notes UI with an error view
// instead of silently falling through to an empty note list.
export const startupError = writable<string | null>(null);
// Off by default - opt-in, mirrors how other background-work features in
// this app (line numbers, updates) are explicit rather than automatic. Only
// ever meaningful with 2+ registered databases.
export const searchAllDatabases = writable(false);
// Cache of every OTHER database's notes, refreshed on toggle and after a
// database switch - NOT refetched per keystroke, so search stays instant
// client-side filtering exactly like the single-database case, just over a
// merged array.
export const otherDatabaseNotes = writable<CrossDatabaseNote[]>([]);

export const activeDatabaseName = derived(
  [databases, activeDatabaseId],
  ([$databases, $activeDatabaseId]) => $databases.find((db) => db.id === $activeDatabaseId)?.name ?? null,
);

export async function refreshOtherDatabaseNotes(): Promise<void> {
  otherDatabaseNotes.set(await databaseService.listNotesFromOtherDatabases());
}

export function toggleSearchAllDatabases(): void {
  const next = !get(searchAllDatabases);
  searchAllDatabases.set(next);
  if (next) void refreshOtherDatabaseNotes();
}

// Applies a fresh AppState after touching the active connection (switching,
// reloading, retrying startup) - `switch_database`/`reload_database`
// resolve successfully even when activation itself failed (e.g. a
// removable drive unplugged mid-action), so `ready` must be checked
// explicitly rather than assumed from the absence of a thrown error.
// Returns whether the database is actually reachable, so the caller knows
// whether to go on and (re)initialize notes.
export function applyDatabaseState(state: AppState | null, unavailableMessage: string): boolean {
  if (state) {
    databases.set(state.databases);
    activeDatabaseId.set(state.activeDatabaseId);
  }
  if (!state || !state.ready) {
    startupError.set(state?.error ?? unavailableMessage);
    return false;
  }
  startupError.set(null);
  return true;
}

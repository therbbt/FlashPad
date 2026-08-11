import { derived, get, writable } from 'svelte/store';
import { DatabaseService, type AppState, type DatabaseProfile, type CrossDatabaseNote } from '../services/databaseService';
import { listMyNotebooks } from '../services/cloudService';
import { CloudNotesService } from '../services/cloudNotesService';
import { notebooks as cloudNotebooks } from './cloudStore';

const databaseService = new DatabaseService();

export const databases = writable<DatabaseProfile[]>([]);
export const activeDatabaseId = writable<number | null>(null);
// Set to the notebook id while a cloud notebook is the active database,
// null whenever a local profile is active - kept separate from
// activeDatabaseId (which only ever holds a local profile's id, and keeps
// remembering it even while cloud is active, since that's where switching
// back to local resumes) since the two id spaces are different types and
// don't overlap.
export const activeCloudNotebookId = writable<string | null>(null);
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

// Reflects whichever database is ACTUALLY active - a cloud notebook's name
// when one is (falling back to a generic label if cloudStore.notebooks
// hasn't been refreshed with it yet), otherwise the active local profile's
// name. Before cloud notebooks existed this only ever needed to look at
// `databases`/`activeDatabaseId`; without the activeCloudNotebookId branch,
// this would keep showing the last-active LOCAL profile's name (stale and
// misleading) the whole time a cloud notebook is actually active.
export const activeDatabaseName = derived(
  [databases, activeDatabaseId, activeCloudNotebookId, cloudNotebooks],
  ([$databases, $activeDatabaseId, $activeCloudNotebookId, $cloudNotebooks]) =>
    $activeCloudNotebookId != null
      ? ($cloudNotebooks.find((nb) => nb.id === $activeCloudNotebookId)?.name ?? 'Cloud notebook')
      : ($databases.find((db) => db.id === $activeDatabaseId)?.name ?? null),
);

// Drives whether the "search all databases" toggle even makes sense to show
// - true once there's at least one OTHER database (local or cloud) to
// search beyond whichever one is currently active. cloudNotebooks only
// reflects reality once something has called cloudStore.refreshNotebooks()
// (App.svelte does this at boot when signed in, and CloudNotebooksSection
// does it whenever it's open) - until then this just falls back to the
// local-only count, same as before cloud notebooks existed.
export const hasSearchableOtherSources = derived(
  [databases, cloudNotebooks],
  ([$databases, $cloudNotebooks]) => $databases.length > 1 || $cloudNotebooks.length > 0,
);

// Merges local profiles' notes (via Rust, same as before) with every OTHER
// cloud notebook the signed-in user belongs to (skipping whichever one is
// currently active, if any) - powers the "search all databases" toggle
// across both kinds of database. Falls back to local-only results if the
// cloud fetch fails (not signed in, cloud not configured, offline), the
// same per-source skip-on-error behavior Rust's own
// list_notes_from_other_databases uses for an unreachable local profile.
export async function refreshOtherDatabaseNotes(): Promise<void> {
  const localOthers = await databaseService.listNotesFromOtherDatabases();

  let cloudOthers: CrossDatabaseNote[] = [];
  try {
    const activeCloudId = get(activeCloudNotebookId);
    const notebooks = await listMyNotebooks();
    const otherNotebooks = notebooks.filter((notebook) => notebook.id !== activeCloudId);
    const perNotebook = await Promise.all(
      otherNotebooks.map(async (notebook) => {
        const notesInNotebook = await new CloudNotesService(notebook.id).list();
        return notesInNotebook.map((note) => ({ ...note, databaseId: notebook.id, databaseName: notebook.name }) as CrossDatabaseNote);
      }),
    );
    cloudOthers = perNotebook.flat();
  } catch {
    // Not signed in, cloud not configured, or offline - local results alone
    // are still a valid (if partial) answer.
  }

  otherDatabaseNotes.set([...localOthers, ...cloudOthers]);
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

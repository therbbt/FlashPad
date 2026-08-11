// Persists which database (a local SQLite profile or a cloud notebook) was
// last active, so the app resumes into the same place on restart. Uses
// localStorage directly, matching the established pattern in this codebase
// (see settingsService.ts's STORAGE_KEY / notesStore.ts's EXPANDED_KEY) -
// NOT @tauri-apps/plugin-store, which is declared in package.json but was
// never actually registered as a Tauri plugin in src-tauri/src/main.rs (no
// tauri-plugin-store in Cargo.toml either), so it isn't functional yet.
// Kept deliberately separate from Rust's flashpad.config.json, which keeps
// managing local profiles exactly as it always has.
export type ActiveSource = { kind: 'local'; id: number } | { kind: 'cloud'; notebookId: string };

const STORAGE_KEY = 'flashpad.activeSource';

export function loadActiveSource(): ActiveSource | null {
  if (typeof window === 'undefined') return null;
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as ActiveSource;
    if (parsed.kind === 'local' && typeof parsed.id === 'number') return parsed;
    if (parsed.kind === 'cloud' && typeof parsed.notebookId === 'string') return parsed;
    return null;
  } catch {
    return null;
  }
}

export function saveActiveSource(source: ActiveSource): void {
  if (typeof window === 'undefined') return;
  window.localStorage.setItem(STORAGE_KEY, JSON.stringify(source));
}

export function clearActiveSource(): void {
  if (typeof window === 'undefined') return;
  window.localStorage.removeItem(STORAGE_KEY);
}

import { derived, get, writable } from 'svelte/store';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { NotesService, type NoteRecord, type NotesBackend } from '../services/notesService';
import type { TreeItem } from '../components/TreeNode.svelte';
import type { ContextMenuItem } from '../components/ContextMenu.svelte';
import { status } from './statusStore';

// The local Tauri/SQLite backend, kept as its own reference (not just the
// initial value of activeBackend below) because importFlashNoteFolder is
// local-only - it reads files from disk via Tauri's dialog+fs directly in
// Rust and was never made part of NotesBackend, so it must always be called
// against this instance regardless of which backend is currently active.
const localNotesService = new NotesService();
let activeBackend: NotesBackend = localNotesService;

// Lets App.svelte (database switching) and the cloud notebook UI repoint
// every note operation below at either the local Tauri backend or a
// Supabase-backed CloudNotesService, without notesStore's own call sites
// needing to know which one is live.
export function setActiveNotesBackend(backend: NotesBackend): void {
  activeBackend = backend;
}

// Convenience for switching back to the local backend, so App.svelte never
// needs its own NotesService reference (or import) just to call
// setActiveNotesBackend(new NotesService()) - reuses the same
// localNotesService instance every time rather than constructing a new one
// per switch.
export function useLocalNotesBackend(): void {
  activeBackend = localNotesService;
}

export function isCloudBackendActive(): boolean {
  return activeBackend !== localNotesService;
}

const EXPANDED_KEY = 'flashpad.expandedFolders';

const loadExpanded = (): Set<number> => {
  if (typeof window === 'undefined') return new Set();
  try {
    const raw = window.localStorage.getItem(EXPANDED_KEY);
    return raw ? new Set(JSON.parse(raw) as number[]) : new Set();
  } catch {
    return new Set();
  }
};

const saveExpanded = (value: Set<number>) => {
  if (typeof window !== 'undefined') {
    window.localStorage.setItem(EXPANDED_KEY, JSON.stringify([...value]));
  }
};

// Wraps a plain writable so every set/update automatically persists to
// localStorage - callers never need to remember a separate saveExpanded()
// call (a real omission in the pre-store code, e.g. easy to forget on a new
// call site).
function persistedExpandedNotes() {
  const store = writable<Set<number>>(loadExpanded());
  return {
    subscribe: store.subscribe,
    set: (value: Set<number>) => {
      saveExpanded(value);
      store.set(value);
    },
    update: (fn: (value: Set<number>) => Set<number>) => {
      store.update((value) => {
        const next = fn(value);
        saveExpanded(next);
        return next;
      });
    },
  };
}

// ---------- state ----------

export const notes = writable<NoteRecord[]>([]);
export const selectedId = writable<number | null>(null);
export const activeParentId = writable<number | null>(null);
export const expandedNotes = persistedExpandedNotes();
export const focusedKey = writable<string | null>(null);
export const renamingKey = writable<string | null>(null);
export const draggingId = writable<number | null>(null);
// The app's own internal copy/move clipboard for notes - unrelated to the
// OS clipboard used by NoteInfoPopover/link-copying.
export const clipboard = writable<{ id: number; mode: 'copy' | 'cut' } | null>(null);

// ---------- tree construction ----------

export const buildTree = (noteList: NoteRecord[]): TreeItem[] => {
  const nodeById = new Map<number, TreeItem>();
  noteList.forEach((n) => nodeById.set(n.id, { id: n.id, title: n.title, children: [], isMarkdown: n.isMarkdown, isLocked: n.isLocked, createdAt: n.createdAt, sortOrder: n.sortOrder }));

  const roots: TreeItem[] = [];
  noteList.forEach((n) => {
    const node = nodeById.get(n.id)!;
    const parent = n.parentId != null ? nodeById.get(n.parentId) : undefined;
    if (parent) parent.children.push(node);
    else roots.push(node);
  });

  // sortOrder is the single source of truth for tree order - it starts out
  // equivalent to creation order (see migrate_add_sort_order_column and
  // next_sort_order in the Rust backend) and is only changed by dragging a
  // note to reorder or renest it.
  const sortItems = (items: TreeItem[]) => {
    items.sort((a, b) => a.sortOrder - b.sortOrder);
    items.forEach((item) => sortItems(item.children));
  };
  sortItems(roots);
  return roots;
};

export const flattenVisible = (items: TreeItem[], expanded: Set<number>): { key: string; item: TreeItem }[] => {
  const out: { key: string; item: TreeItem }[] = [];
  const walk = (list: TreeItem[]) => {
    for (const item of list) {
      const key = `note:${item.id}`;
      out.push({ key, item });
      if (item.children.length && expanded.has(item.id)) walk(item.children);
    }
  };
  walk(items);
  return out;
};

const notePath = (note: NoteRecord, noteList: NoteRecord[]): string => {
  const parts: string[] = [note.title];
  let current: NoteRecord | undefined = note;
  while (current && current.parentId != null) {
    current = noteList.find((n) => n.id === current!.parentId);
    if (current) parts.unshift(current.title);
  }
  return parts.join(' / ');
};

export const collectDescendantNoteIds = (rootId: number): Set<number> => {
  const noteList = get(notes);
  const ids = new Set<number>();
  const queue = [rootId];
  while (queue.length) {
    const current = queue.pop()!;
    for (const n of noteList) {
      if (n.parentId === current && !ids.has(n.id)) {
        ids.add(n.id);
        queue.push(n.id);
      }
    }
  }
  return ids;
};

export const tree = derived(notes, ($notes) => buildTree($notes));

export const dropDisabledIds = derived([draggingId, notes], ([$draggingId]) =>
  $draggingId != null ? new Set([$draggingId, ...collectDescendantNoteIds($draggingId)]) : new Set<number>(),
);

// ---------- data loading ----------

export async function refreshNotes(): Promise<void> {
  notes.set(await activeBackend.list());
}

export async function refreshAll(): Promise<void> {
  await refreshNotes();
  status.set('Refreshed');
}

// Replaces one note in place after a save that doesn't otherwise change
// selection/tree shape (editor autosave, Markdown-mode toggle) - the
// caller already has the saved record back from the backend.
export function applyUpdatedNote(saved: NoteRecord): void {
  notes.update((list) => list.map((n) => (n.id === saved.id ? saved : n)));
}

// Thin passthroughs so App.svelte's editor autosave / markdown-toggle paths
// never hold their own NotesService reference - routing them through here
// means setActiveNotesBackend is the single place backend selection lives,
// with no second copy of the app that could keep writing to the wrong
// database after switching to a cloud notebook.
export const saveNote = (note: Parameters<NotesBackend['save']>[0]): Promise<NoteRecord> => activeBackend.save(note);
export const saveChecklistToggle = (id: number, content: string): Promise<NoteRecord> => activeBackend.saveChecklistToggle(id, content);

// Clears everything scoped to the previously-active database's notes -
// called when switching databases so no stale ids from the old vault leak
// into tree-expansion or clipboard state. Query (a different, search-domain
// concern) is reset by the caller.
export function resetSelection(): void {
  selectedId.set(null);
  activeParentId.set(null);
  expandedNotes.set(new Set());
  clipboard.set(null);
}

export function collapseAll(): void {
  expandedNotes.set(new Set());
}

// ---------- creation ----------

export async function createNoteIn(parentId: number | null, defaultTitle = 'Untitled'): Promise<NoteRecord> {
  const created = await activeBackend.create({ title: defaultTitle, content: '', parentId });
  notes.update((list) => [created, ...list]);
  if (parentId != null) {
    expandedNotes.update((set) => (set.has(parentId) ? set : new Set(set).add(parentId)));
  }
  focusedKey.set(`note:${created.id}`);
  status.set('New note');
  return created;
}

// Imports a FlashNote export (a different, unrelated app): folders become
// subnotes, .txt files become notes, and a "<folder>.txt" sibling next to a
// folder becomes that subnote's own content. The picked folder's own
// subfolders/files land as new top-level notes directly - no extra wrapper
// note for the picked folder itself.
//
// Triggered from Settings, which shows its own inline "Importing…"/result
// state (same pattern as the "Check for updates" button) rather than
// reporting through the main window's status bar - errors are left to
// propagate so Settings can display them. Returns null if the user
// cancelled the folder picker, distinct from a real failure.
export async function importFromFolder(): Promise<{ imported: NoteRecord | null; importedCount: number } | null> {
  // Local-only: it reads files from disk via Tauri directly into whichever
  // local SQLite profile is open, so running it while a cloud notebook is
  // active would silently write into a local database the user isn't even
  // looking at. Settings should already hide this action in that state -
  // this is the defense-in-depth backstop.
  if (isCloudBackendActive()) {
    throw new Error('Importing from a folder only works with a local database - switch to one first.');
  }
  const picked = await openDialog({ directory: true, title: 'Select a FlashNote export folder' });
  if (typeof picked !== 'string') return null;

  const summary = await localNotesService.importFlashNoteFolder(picked);
  await refreshNotes();
  const imported = summary.firstNoteId != null ? get(notes).find((n) => n.id === summary.firstNoteId) ?? null : null;
  if (imported) focusedKey.set(`note:${imported.id}`);
  return { imported, importedCount: summary.importedCount };
}

// Only used once, when the database is empty (first launch / fresh
// install) - gives a new user something to look at instead of a blank
// untitled note, and doubles as a quick reference for the core shortcuts.
export async function createWelcomeNote(hotkeyLabel: string): Promise<NoteRecord> {
  const content = [
    '# Welcome to FlashPad',
    '',
    `Press **${hotkeyLabel}** anywhere to open FlashPad instantly.`,
    'Press **Esc** to hide it - it keeps running in the tray.',
    '',
    '## Notes & subnotes',
    '',
    'Right-click a note (or the sidebar background) to create a note, rename, duplicate, move, or delete. Any note can hold subnotes - once it has one, it shows a folder icon: click it to open its own content, click the little arrow to expand or collapse its subnotes.',
    '',
    '## Markdown',
    '',
    'Toggle **Markdown** at the bottom of a note to format as you type - headings, **bold**, lists, and more. Use the Markdown guide button (top right) for the full syntax. Links open in your default browser - Ctrl+Click (or a plain click on a locked note), right-click for more options, or Alt+O to open the link under the caret.',
    '',
    '## Locking notes',
    '',
    "Right-click a note's text (or press **Alt+L**) to lock it - a locked note can't be edited until you unlock it again.",
    '',
    '## Search',
    '',
    'Use the search box at the bottom to find notes, with prev/next buttons (or **Enter** / **Shift+Enter**) to step through matches.',
    '',
    '## Vim mode',
    '',
    "Turn on **Vim mode** in Settings for modal editing - off by default. Plain text notes get full vim motions, operators, and search. Markdown notes get a smaller set (normal/insert modes, **h j k l**, **0**/**$**, **i**/**o**/**O**, **x**, **dd**). The sidebar also supports **j**/**k** to move focus when it's on. The current mode shows in the footer.",
    '',
    '## Shortcuts',
    '',
    '- **Alt+N** - Create a new note',
    '- **Alt+L** - Lock / unlock the current note',
    '- **Alt+D** - Delete the current note (and its subnotes)',
    '- **Alt+M** - Toggle Markdown view',
    '- **Alt+R** - Toggle line numbers (plain text notes)',
    '- **Alt+↑ / Alt+↓** - Move the current note up/down among its siblings',
    '- **Alt+→** - Nest the current note under its previous sibling',
    "- **Alt+←** - Move the current note out to its parent's level",
    '- **Alt+O** - Open the link under the caret (Markdown view)',
    '- **Alt+B** - Switch to the next database',
    '- **Alt+T** - Toggle focus between the editor and the notes menu',
    '- **Alt+1** - Insert a divider',
    '- **Alt+2** - Insert a timestamp',
    '- **Alt+3** - Insert a dateline',
    '',
    '## Settings',
    '',
    'The gear icon in Settings lets you launch FlashPad at login and change the hotkey above to whatever you like.',
    '',
    '---',
    '',
    '*Start typing to replace this note.*',
  ].join('\n');

  const created = await activeBackend.create({ title: 'Welcome to FlashPad', content, parentId: null, isMarkdown: true });
  notes.update((list) => [created, ...list]);
  focusedKey.set(`note:${created.id}`);
  return created;
}

// ---------- rename / move / delete / duplicate ----------

export async function commitRename(key: string, value: string): Promise<{ id: number; updated: NoteRecord } | null> {
  renamingKey.set(null);
  const trimmed = value.trim();
  if (!trimmed) return null;

  const id = Number(key.slice('note:'.length));
  if (get(notes).find((n) => n.id === id)?.isLocked) return null;
  const updated = await activeBackend.save({ id, title: trimmed });
  notes.update((list) => list.map((n) => (n.id === id ? updated : n)));
  return { id, updated };
}

export function buildMoveTargetItems(onPick: (parentId: number | null) => void, excludeId?: number): ContextMenuItem[] {
  const noteList = get(notes);
  const descendantIds = excludeId != null ? collectDescendantNoteIds(excludeId) : new Set<number>();
  const eligible = noteList
    .filter((n) => n.id !== excludeId && !descendantIds.has(n.id))
    .sort((a, b) => notePath(a, noteList).localeCompare(notePath(b, noteList), undefined, { sensitivity: 'base' }));
  return [
    { label: 'Notes (root)', action: () => onPick(null) },
    ...eligible.map((n) => ({ label: notePath(n, noteList), action: () => onPick(n.id) })),
  ];
}

export async function moveNoteTo(id: number, parentId: number | null): Promise<boolean> {
  try {
    const updated = await activeBackend.move(id, parentId);
    notes.update((list) => list.map((n) => (n.id === id ? updated : n)));
    if (get(selectedId) === id) activeParentId.set(parentId);
    status.set('Moved');
    return true;
  } catch (err) {
    status.set(err instanceof Error ? err.message : 'Move failed');
    return false;
  }
}

// ---------- drag-and-drop tree reordering ----------

export const onDragStartRow = (id: number) => draggingId.set(id);
export const onDragEndRow = () => draggingId.set(null);
export const onDropRow = (draggedId: number, targetId: number, zone: 'before' | 'inside' | 'after') => void handleTreeDrop(draggedId, targetId, zone);

export async function handleTreeDrop(draggedId: number, targetId: number, zone: 'before' | 'inside' | 'after'): Promise<void> {
  if (draggedId === targetId || get(dropDisabledIds).has(targetId)) return;
  const noteList = get(notes);
  const target = noteList.find((n) => n.id === targetId);
  if (!target) return;

  let parentId: number | null;
  let beforeId: number | null;
  if (zone === 'inside') {
    parentId = target.id;
    beforeId = null;
  } else {
    parentId = target.parentId;
    const siblings = noteList
      .filter((n) => n.parentId === target.parentId && n.id !== draggedId)
      .sort((a, b) => a.sortOrder - b.sortOrder);
    const targetIndex = siblings.findIndex((n) => n.id === targetId);
    beforeId = zone === 'before' ? targetId : (siblings[targetIndex + 1]?.id ?? null);
  }

  try {
    await activeBackend.reorder(draggedId, parentId, beforeId);
    await refreshNotes();
    status.set('Reordered');
  } catch (err) {
    status.set(err instanceof Error ? err.message : 'Reorder failed');
  }
}

// Keyboard alternative to dragging a row (Alt+ArrowUp/ArrowDown) - swaps a
// note with its immediately preceding/following sibling. Works identically
// on every platform, unlike HTML5 drag-and-drop (which Tauri's native
// drag-drop handling breaks on Windows unless disabled - see
// src-tauri/tauri.windows.conf.json).
export async function moveNoteOrder(id: number, direction: -1 | 1): Promise<void> {
  const noteList = get(notes);
  const note = noteList.find((n) => n.id === id);
  if (!note) return;
  const siblings = noteList.filter((n) => n.parentId === note.parentId).sort((a, b) => a.sortOrder - b.sortOrder);
  const index = siblings.findIndex((n) => n.id === id);
  const targetIndex = index + direction;
  if (index === -1 || targetIndex < 0 || targetIndex >= siblings.length) return;

  const beforeId = direction < 0 ? siblings[targetIndex].id : (siblings[targetIndex + 1]?.id ?? null);

  try {
    await activeBackend.reorder(id, note.parentId, beforeId);
    await refreshNotes();
    status.set('Reordered');
  } catch (err) {
    status.set(err instanceof Error ? err.message : 'Reorder failed');
  }
}

// Keyboard alternative to dragging a row onto/out of a note (Alt+ArrowLeft/
// ArrowRight) - complements moveNoteOrder (Alt+ArrowUp/Down, which only
// reorders among the current siblings) by changing nesting level, the same
// indent/outdent convention most outliners use.

// Nests the note under its immediately preceding sibling, as that
// sibling's new last child - a no-op if it's already the first among its
// siblings (nothing above it to nest under).
export async function indentNote(id: number): Promise<void> {
  const noteList = get(notes);
  const note = noteList.find((n) => n.id === id);
  if (!note) return;
  const siblings = noteList.filter((n) => n.parentId === note.parentId).sort((a, b) => a.sortOrder - b.sortOrder);
  const index = siblings.findIndex((n) => n.id === id);
  if (index <= 0) return;
  const newParent = siblings[index - 1];

  try {
    await activeBackend.reorder(id, newParent.id, null);
    // The note would otherwise vanish from view if its new parent is
    // currently collapsed.
    expandedNotes.update((set) => (set.has(newParent.id) ? set : new Set(set).add(newParent.id)));
    if (get(selectedId) === id) activeParentId.set(newParent.id);
    await refreshNotes();
    status.set('Moved in');
  } catch (err) {
    status.set(err instanceof Error ? err.message : 'Move failed');
  }
}

// Promotes the note to be its parent's own next sibling (one level up,
// positioned right after the former parent) - a no-op if it's already at
// the root.
export async function outdentNote(id: number): Promise<void> {
  const noteList = get(notes);
  const note = noteList.find((n) => n.id === id);
  if (!note || note.parentId == null) return;
  const parent = noteList.find((n) => n.id === note.parentId);
  if (!parent) return;
  const grandparentId = parent.parentId;
  const grandSiblings = noteList.filter((n) => n.parentId === grandparentId).sort((a, b) => a.sortOrder - b.sortOrder);
  const parentIndex = grandSiblings.findIndex((n) => n.id === parent.id);
  const beforeId = grandSiblings[parentIndex + 1]?.id ?? null;

  try {
    await activeBackend.reorder(id, grandparentId, beforeId);
    if (get(selectedId) === id) activeParentId.set(grandparentId);
    await refreshNotes();
    status.set('Moved out');
  } catch (err) {
    status.set(err instanceof Error ? err.message : 'Move failed');
  }
}

export async function duplicateNote(id: number): Promise<NoteRecord> {
  const created = await activeBackend.duplicate(id);
  notes.update((list) => [created, ...list]);
  status.set('Duplicated');
  return created;
}

export async function toggleLock(id: number): Promise<NoteRecord | null> {
  const note = get(notes).find((n) => n.id === id);
  if (!note) return null;
  const next = !note.isLocked;
  const saved = await activeBackend.save({ id, isLocked: next });
  notes.update((list) => list.map((n) => (n.id === saved.id ? saved : n)));
  status.set(next ? 'Locked' : 'Unlocked');
  return saved;
}

export const copyNote = (id: number) => {
  clipboard.set({ id, mode: 'copy' });
  status.set('Copied');
};

export const cutNote = (id: number) => {
  clipboard.set({ id, mode: 'cut' });
  status.set('Cut');
};

export async function pasteNote(targetParentId: number | null): Promise<void> {
  const current = get(clipboard);
  if (!current) return;
  const { id, mode } = current;
  if (mode === 'copy') {
    const created = await activeBackend.duplicate(id);
    notes.update((list) => [created, ...list]);
    await moveNoteTo(created.id, targetParentId);
  } else {
    const moved = await moveNoteTo(id, targetParentId);
    if (moved) clipboard.set(null);
  }
}

export async function deleteNote(id: number): Promise<{ removedSelected: boolean; nextNote: NoteRecord | null }> {
  const descendantIds = collectDescendantNoteIds(id);
  const removedIds = new Set([id, ...descendantIds]);

  await activeBackend.delete(id);
  const remaining = get(notes).filter((n) => !removedIds.has(n.id));
  notes.set(remaining);

  let removedSelected = false;
  let nextNote: NoteRecord | null = null;
  if (get(selectedId) != null && removedIds.has(get(selectedId)!)) {
    removedSelected = true;
    if (remaining.length) {
      nextNote = remaining[0];
      selectedId.set(remaining[0].id);
    } else {
      selectedId.set(null);
    }
  }
  if (get(activeParentId) != null && removedIds.has(get(activeParentId)!)) {
    activeParentId.set(null);
  }
  status.set('Deleted');
  return { removedSelected, nextNote };
}

// ---------- tree state ----------

// Deliberately doesn't touch activeParentId: expanding/collapsing a note to
// browse its children shouldn't change where "New note" lands - that's
// driven only by whichever note you actually have open (selection is owned
// by App.svelte's selectNote, which sets activeParentId directly).
export function toggleExpand(id: number): void {
  expandedNotes.update((set) => {
    const next = new Set(set);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    return next;
  });
}

export async function toggleLineNumbers(id: number): Promise<NoteRecord | null> {
  const note = get(notes).find((n) => n.id === id);
  if (!note) return null;
  const next = !note.showLineNumbers;
  const saved = await activeBackend.save({ id, showLineNumbers: next });
  notes.update((list) => list.map((n) => (n.id === saved.id ? saved : n)));
  status.set(next ? 'Line numbers on' : 'Line numbers off');
  return saved;
}

// Per-note toggle for the syntax-highlighted CodeMirror view (see
// EditorModeEditor.svelte) - same shape as toggleLock/toggleLineNumbers.
export async function toggleEditorMode(id: number): Promise<NoteRecord | null> {
  const note = get(notes).find((n) => n.id === id);
  if (!note) return null;
  const next = !note.isEditorMode;
  const saved = await activeBackend.save({ id, isEditorMode: next });
  notes.update((list) => list.map((n) => (n.id === saved.id ? saved : n)));
  status.set(next ? 'Editor mode on' : 'Editor mode off');
  return saved;
}

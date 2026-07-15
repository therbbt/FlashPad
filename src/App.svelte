<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { NotesService, type NoteRecord } from './lib/services/notesService';
  import { SettingsService, type FlashPadSettings } from './lib/services/settingsService';
  import { DEFAULT_DARK_PALETTE_ID, DEFAULT_LIGHT_PALETTE_ID, applyPalette, getPalette } from './lib/theme/palettes';
  import { HotkeyService } from './lib/services/hotkeyService';
  import { DatabaseService, type AppState } from './lib/services/databaseService';
  import TreeNode, { type TreeItem } from './lib/components/TreeNode.svelte';
  import ContextMenu, { type ContextMenuItem } from './lib/components/ContextMenu.svelte';
  import ShortcutsPanel from './lib/components/ShortcutsPanel.svelte';
  import SettingsPanel from './lib/components/SettingsPanel.svelte';
  import MarkdownEditor from './lib/components/MarkdownEditor.svelte';
  import MarkdownHelpPanel from './lib/components/MarkdownHelpPanel.svelte';
  import ConfirmDialog from './lib/components/ConfirmDialog.svelte';
  import TitleBar from './lib/components/TitleBar.svelte';
  import ResizeHandles from './lib/components/ResizeHandles.svelte';

  const notesService = new NotesService();
  const settingsService = new SettingsService();
  const hotkeyService = new HotkeyService();
  const databaseService = new DatabaseService();

  const EXPANDED_KEY = 'flashpad.expandedFolders';
  const SIDEBAR_WIDTH_KEY = 'flashpad.sidebarWidth';
  const SIDEBAR_MIN_WIDTH = 80;
  const SIDEBAR_MAX_WIDTH = 480;
  const DEFAULT_SIDEBAR_WIDTH = 260;

  let notes: NoteRecord[] = [];
  let selectedId: number | null = null;
  let activeParentId: number | null = null;
  let expandedNotes: Set<number> = new Set();
  let focusedKey: string | null = null;
  let renamingKey: string | null = null;
  let contextMenu: { x: number; y: number; items: ContextMenuItem[] } | null = null;
  let confirmState: { message: string; resolve: (value: boolean) => void } | null = null;
  let clipboard: { id: number; mode: 'copy' | 'cut' } | null = null;
  let shortcutsOpen = false;
  let settingsOpen = false;
  let markdownHelpOpen = false;
  // Set when the configured active database is unreachable at startup (e.g.
  // an unmounted sync folder) - replaces the notes UI with an error view
  // instead of silently falling through to an empty note list.
  let startupError: string | null = null;
  let hotkeySetting = 'Alt+S';
  let sidebarWidth = DEFAULT_SIDEBAR_WIDTH;
  let isResizingSidebar = false;

  let noteText = '';
  let title = 'Untitled';
  let titleAutoDerive = true;
  let query = '';
  let status = 'Ready';
  let theme: FlashPadSettings['theme'] = 'dark';
  let lightPaletteId = DEFAULT_LIGHT_PALETTE_ID;
  let darkPaletteId = DEFAULT_DARK_PALETTE_ID;
  let isMarkdownActive = false;
  let isLockedActive = false;
  let textarea: HTMLTextAreaElement;
  let markdownEditorRef: MarkdownEditor | undefined;
  let treeEl: HTMLDivElement;
  let insertButton: HTMLButtonElement;
  let notesButton: HTMLButtonElement;
  let saveTimer: ReturnType<typeof setTimeout> | undefined;

  // ---------- persistence helpers ----------

  const loadExpanded = (): Set<number> => {
    if (typeof window === 'undefined') return new Set();
    try {
      const raw = window.localStorage.getItem(EXPANDED_KEY);
      return raw ? new Set(JSON.parse(raw) as number[]) : new Set();
    } catch {
      return new Set();
    }
  };

  const saveExpanded = () => {
    if (typeof window !== 'undefined') {
      window.localStorage.setItem(EXPANDED_KEY, JSON.stringify([...expandedNotes]));
    }
  };

  const loadSidebarWidth = (): number => {
    if (typeof window === 'undefined') return DEFAULT_SIDEBAR_WIDTH;
    const raw = Number(window.localStorage.getItem(SIDEBAR_WIDTH_KEY));
    if (!raw || Number.isNaN(raw)) return DEFAULT_SIDEBAR_WIDTH;
    return Math.min(SIDEBAR_MAX_WIDTH, Math.max(SIDEBAR_MIN_WIDTH, raw));
  };

  const saveSidebarWidth = () => {
    if (typeof window !== 'undefined') {
      window.localStorage.setItem(SIDEBAR_WIDTH_KEY, String(sidebarWidth));
    }
  };

  const startSidebarResize = (event: MouseEvent) => {
    event.preventDefault();
    isResizingSidebar = true;
    document.body.classList.add('resizing-sidebar');

    const handleMove = (moveEvent: MouseEvent) => {
      sidebarWidth = Math.min(SIDEBAR_MAX_WIDTH, Math.max(SIDEBAR_MIN_WIDTH, moveEvent.clientX));
    };

    const handleUp = () => {
      isResizingSidebar = false;
      document.body.classList.remove('resizing-sidebar');
      saveSidebarWidth();
      window.removeEventListener('mousemove', handleMove);
      window.removeEventListener('mouseup', handleUp);
    };

    window.addEventListener('mousemove', handleMove);
    window.addEventListener('mouseup', handleUp);
  };

  // ---------- tree construction ----------

  const buildTree = (noteList: NoteRecord[]): TreeItem[] => {
    const nodeById = new Map<number, TreeItem>();
    noteList.forEach((n) => nodeById.set(n.id, { id: n.id, title: n.title, children: [], isMarkdown: n.isMarkdown, isLocked: n.isLocked, createdAt: n.createdAt, sortOrder: n.sortOrder }));

    const roots: TreeItem[] = [];
    noteList.forEach((n) => {
      const node = nodeById.get(n.id)!;
      const parent = n.parentId != null ? nodeById.get(n.parentId) : undefined;
      if (parent) parent.children.push(node);
      else roots.push(node);
    });

    // sortOrder is the single source of truth for tree order - it starts
    // out equivalent to creation order (see migrate_add_sort_order_column
    // and next_sort_order in the Rust backend) and is only changed by
    // dragging a note to reorder or renest it.
    const sortItems = (items: TreeItem[]) => {
      items.sort((a, b) => a.sortOrder - b.sortOrder);
      items.forEach((item) => sortItems(item.children));
    };
    sortItems(roots);
    return roots;
  };

  const flattenVisible = (items: TreeItem[], expanded: Set<number>): { key: string; item: TreeItem }[] => {
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

  const notePath = (note: NoteRecord): string => {
    const parts: string[] = [note.title];
    let current: NoteRecord | undefined = note;
    while (current && current.parentId != null) {
      current = notes.find((n) => n.id === current!.parentId);
      if (current) parts.unshift(current.title);
    }
    return parts.join(' / ');
  };

  const collectDescendantNoteIds = (rootId: number): Set<number> => {
    const ids = new Set<number>();
    const queue = [rootId];
    while (queue.length) {
      const current = queue.pop()!;
      for (const n of notes) {
        if (n.parentId === current && !ids.has(n.id)) {
          ids.add(n.id);
          queue.push(n.id);
        }
      }
    }
    return ids;
  };

  $: tree = buildTree(notes);
  $: normalizedQuery = query.trim().toLowerCase();
  $: isSearching = normalizedQuery.length > 0;
  $: searchResults = isSearching
    ? notes
        .filter((n) => `${n.title} ${n.content}`.toLowerCase().includes(normalizedQuery))
        .sort((a, b) => a.title.localeCompare(b.title, undefined, { sensitivity: 'base' }))
    : [];
  $: visibleFlat = isSearching
    ? searchResults.map((n) => ({ key: `note:${n.id}`, item: { id: n.id, title: n.title, children: [], isMarkdown: n.isMarkdown, isLocked: n.isLocked, createdAt: n.createdAt, sortOrder: n.sortOrder } as TreeItem }))
    : flattenVisible(tree, expandedNotes);
  $: if (visibleFlat.length && !visibleFlat.some((v) => v.key === focusedKey)) {
    focusedKey = visibleFlat[0].key;
  }
  $: currentPath = activeParentId != null ? (notes.find((n) => n.id === activeParentId) ? notePath(notes.find((n) => n.id === activeParentId)!) : 'Notes') : 'Notes';
  $: searchMatchIndex = isSearching ? searchResults.findIndex((n) => n.id === selectedId) : -1;
  $: selectedNoteCreatedAt = notes.find((n) => n.id === selectedId)?.createdAt ?? null;

  // ---------- data loading ----------

  const refreshNotes = async () => {
    notes = await notesService.list();
  };

  const refreshAll = async () => {
    await refreshNotes();
    status = 'Refreshed';
  };

  // Loads notes for whichever database is currently active and selects
  // something to show. Extracted out of onMount so switching databases (or
  // importing into the active one) can re-run exactly the same startup
  // sequence without a full app reload.
  const initializeNotes = async () => {
    await refreshAll();
    if (notes.length) {
      selectNote(notes[0]);
    } else {
      await createWelcomeNote();
    }
    requestAnimationFrame(() => textarea?.focus());
  };

  // Resets everything scoped to the previously-active database's notes so
  // no stale ids from the old vault leak into tree-expansion, clipboard, or
  // search state after switching to a different database.
  const resetNoteScopedState = () => {
    selectedId = null;
    activeParentId = null;
    expandedNotes = new Set();
    clipboard = null;
    query = '';
  };

  // Shared by every path that can hand back a fresh AppState after
  // touching the active connection (switching, reloading, retrying startup)
  // - `switch_database`/`reload_database` resolve successfully even when
  // activation itself failed (e.g. a removable drive unplugged mid-action),
  // so `ready` must be checked explicitly rather than assumed from the
  // absence of a thrown error.
  const applyAppState = async (state: AppState | null, unavailableMessage: string) => {
    if (!state || !state.ready) {
      startupError = state?.error ?? unavailableMessage;
      return;
    }
    startupError = null;
    resetNoteScopedState();
    await initializeNotes();
  };

  const switchToDatabase = async (id: number) => {
    const state = await databaseService.switchDatabase(id);
    await applyAppState(state, 'The selected database is unavailable.');
  };

  const handleDatabaseReloaded = async (state: AppState) => {
    await applyAppState(state, 'The database is unavailable.');
  };

  const retryStartup = async () => {
    const state = await databaseService.getAppState();
    await applyAppState(state, 'The configured database is unavailable.');
  };

  // ---------- note editor ----------

  const deriveTitleFromContent = (content: string, isMarkdown: boolean): string => {
    const firstLine = content.split('\n').find((line) => line.trim().length > 0)?.trim() ?? '';
    const cleaned = isMarkdown ? firstLine.replace(/^#{1,6}\s+/, '') : firstLine;
    if (!cleaned) return 'Untitled';
    return cleaned.length > 80 ? cleaned.slice(0, 80) : cleaned;
  };

  const selectNote = (note: NoteRecord, focusEditor = true) => {
    selectedId = note.id;
    activeParentId = note.parentId;
    title = note.title;
    noteText = note.content;
    isMarkdownActive = note.isMarkdown;
    isLockedActive = note.isLocked;
    titleAutoDerive = note.title === 'Untitled' || note.title.trim() === '';
    if (focusEditor) requestAnimationFrame(() => textarea?.focus());
  };

  // focusEditor defaults to false here: opening a note from the sidebar (click,
  // search nav) should keep keyboard focus in the tree/search box so arrow-key
  // navigation keeps working. Pass true for deliberate "open to edit" actions
  // (Enter, context menu "Open").
  const openNote = async (id: number, focusEditor = false) => {
    const note = notes.find((n) => n.id === id);
    if (note) selectNote(note, focusEditor);
  };

  const goToSearchMatch = (direction: 1 | -1) => {
    if (!searchResults.length) return;
    const nextIndex = searchMatchIndex === -1
      ? 0
      : (searchMatchIndex + direction + searchResults.length) % searchResults.length;
    const match = searchResults[nextIndex];
    focusedKey = `note:${match.id}`;
    void openNote(match.id);
  };

  const saveActiveNote = async () => {
    if (!selectedId) return;
    const saved = await notesService.save({ id: selectedId, title, content: noteText });
    notes = notes.map((note) => (note.id === saved.id ? saved : note));
    status = 'Saved';
  };

  const scheduleSave = () => {
    status = 'Saving…';
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      void saveActiveNote();
    }, 250);
  };

  const handleEditorInput = () => {
    if (isLockedActive) return;
    if (titleAutoDerive) {
      title = deriveTitleFromContent(noteText, isMarkdownActive);
    }
    scheduleSave();
  };

  const handleMarkdownEditorUpdate = (markdown: string) => {
    if (isLockedActive) return;
    noteText = markdown;
    handleEditorInput();
  };

  const handleTitleInput = () => {
    if (isLockedActive) return;
    titleAutoDerive = false;
    scheduleSave();
  };

  const toggleMarkdown = () => {
    if (selectedId == null || isLockedActive) return;
    const next = !isMarkdownActive;
    isMarkdownActive = next;
    void notesService.save({ id: selectedId, isMarkdown: next }).then((saved) => {
      notes = notes.map((note) => (note.id === saved.id ? saved : note));
    });
  };

  const insertAtCursor = (text: string) => {
    if (isLockedActive) return;
    if (isMarkdownActive) {
      markdownEditorRef?.insertAtCursor(text);
      return;
    }
    const start = textarea.selectionStart;
    const end = textarea.selectionEnd;
    noteText = `${noteText.slice(0, start)}${text}${noteText.slice(end)}`;
    requestAnimationFrame(() => {
      const cursor = start + text.length;
      textarea.focus();
      textarea.setSelectionRange(cursor, cursor);
    });
    handleEditorInput();
  };

  const insertNewline = () => insertAtCursor('-=-=-=-=-=-=-=-=-= =-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-\n');

  const formatLocalTimestamp = (date: Date): string => {
    const pad = (n: number) => String(n).padStart(2, '0');
    const y = date.getFullYear();
    const mo = pad(date.getMonth() + 1);
    const d = pad(date.getDate());
    const h = pad(date.getHours());
    const mi = pad(date.getMinutes());
    const s = pad(date.getSeconds());
    return `${y}-${mo}-${d} ${h}:${mi}:${s}`;
  };

  const insertTimestamp = () => {
    insertAtCursor(`${formatLocalTimestamp(new Date())}\n`);
  };

  const insertDateline = () => {
    const stamp = formatLocalTimestamp(new Date());
    insertAtCursor(`=-=-=-=-=-=-=-=-=-   ${stamp}   -=-=-=-=-=-=-=-=-=\n`);
  };

  // ---------- creation ----------

  const createNoteIn = async (parentId: number | null) => {
    const created = await notesService.create({ title: 'Untitled', content: '', parentId });
    notes = [created, ...notes];
    if (parentId != null && !expandedNotes.has(parentId)) {
      expandedNotes = new Set(expandedNotes).add(parentId);
      saveExpanded();
    }
    selectNote(created);
    focusedKey = `note:${created.id}`;
    status = 'New note';
  };

  // Only used once, when the database is empty (first launch / fresh
  // install) - gives a new user something to look at instead of a blank
  // untitled note, and doubles as a quick reference for the core shortcuts.
  const createWelcomeNote = async () => {
    const content = [
      '# Welcome to FlashPad',
      '',
      `Press **${hotkeySetting}** anywhere to open FlashPad instantly.`,
      'Press **Esc** to hide it - it keeps running in the tray.',
      '',
      '## Notes & subnotes',
      '',
      'Right-click a note (or the sidebar background) to create a note, rename, duplicate, move, or delete. Any note can hold subnotes - once it has one, it shows a folder icon: click it to open its own content, click the little arrow to expand or collapse its subnotes.',
      '',
      '## Markdown',
      '',
      'Toggle **Markdown** at the bottom of a note to format as you type - headings, **bold**, lists, and more. Use the Markdown guide button (top right) for the full syntax.',
      '',
      '## Locking notes',
      '',
      "Right-click a note's text (or press **Alt+L**) to lock it - a locked note can't be edited until you unlock it again.",
      '',
      '## Search',
      '',
      'Use the search box at the bottom to find notes, with prev/next buttons (or **Enter** / **Shift+Enter**) to step through matches.',
      '',
      '## Shortcuts',
      '',
      '- **Alt+N** - Create a new note',
      '- **Alt+L** - Lock / unlock the current note',
      '- **Alt+D** - Delete the current note (and its subnotes)',
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

    const created = await notesService.create({ title: 'Welcome to FlashPad', content, parentId: null, isMarkdown: true });
    notes = [created, ...notes];
    selectNote(created);
    focusedKey = `note:${created.id}`;
  };

  // ---------- rename / move / delete / duplicate ----------

  const commitRename = async (key: string, value: string) => {
    renamingKey = null;
    const trimmed = value.trim();
    if (!trimmed) return;

    const id = Number(key.slice('note:'.length));
    if (notes.find((n) => n.id === id)?.isLocked) return;
    const updated = await notesService.save({ id, title: trimmed });
    notes = notes.map((n) => (n.id === id ? updated : n));
    if (selectedId === id) {
      title = trimmed;
      titleAutoDerive = false;
    }
  };

  const buildMoveTargetItems = (onPick: (parentId: number | null) => void, excludeId?: number): ContextMenuItem[] => {
    const descendantIds = excludeId != null ? collectDescendantNoteIds(excludeId) : new Set<number>();
    const eligible = notes
      .filter((n) => n.id !== excludeId && !descendantIds.has(n.id))
      .sort((a, b) => notePath(a).localeCompare(notePath(b), undefined, { sensitivity: 'base' }));
    return [
      { label: 'Notes (root)', action: () => onPick(null) },
      ...eligible.map((n) => ({ label: notePath(n), action: () => onPick(n.id) })),
    ];
  };

  const moveNoteTo = async (id: number, parentId: number | null): Promise<boolean> => {
    try {
      const updated = await notesService.move(id, parentId);
      notes = notes.map((n) => (n.id === id ? updated : n));
      if (selectedId === id) activeParentId = parentId;
      status = 'Moved';
      return true;
    } catch (err) {
      status = err instanceof Error ? err.message : 'Move failed';
      return false;
    }
  };

  // ---------- drag-and-drop tree reordering ----------

  let draggingId: number | null = null;
  $: dropDisabledIds = draggingId != null ? new Set([draggingId, ...collectDescendantNoteIds(draggingId)]) : new Set<number>();

  const onDragStartRow = (id: number) => (draggingId = id);
  const onDragEndRow = () => (draggingId = null);
  const onDropRow = (draggedId: number, targetId: number, zone: 'before' | 'inside' | 'after') => void handleTreeDrop(draggedId, targetId, zone);

  const handleTreeDrop = async (draggedId: number, targetId: number, zone: 'before' | 'inside' | 'after') => {
    if (draggedId === targetId || dropDisabledIds.has(targetId)) return;
    const target = notes.find((n) => n.id === targetId);
    if (!target) return;

    let parentId: number | null;
    let beforeId: number | null;
    if (zone === 'inside') {
      parentId = target.id;
      beforeId = null;
    } else {
      parentId = target.parentId;
      const siblings = notes
        .filter((n) => n.parentId === target.parentId && n.id !== draggedId)
        .sort((a, b) => a.sortOrder - b.sortOrder);
      const targetIndex = siblings.findIndex((n) => n.id === targetId);
      beforeId = zone === 'before' ? targetId : (siblings[targetIndex + 1]?.id ?? null);
    }

    try {
      await notesService.reorder(draggedId, parentId, beforeId);
      await refreshNotes();
      status = 'Reordered';
    } catch (err) {
      status = err instanceof Error ? err.message : 'Reorder failed';
    }
  };

  const duplicateNote = async (id: number) => {
    const created = await notesService.duplicate(id);
    notes = [created, ...notes];
    selectNote(created);
    status = 'Duplicated';
  };

  const toggleLock = async (id: number) => {
    const note = notes.find((n) => n.id === id);
    if (!note) return;
    const next = !note.isLocked;
    const saved = await notesService.save({ id, isLocked: next });
    notes = notes.map((n) => (n.id === saved.id ? saved : n));
    if (selectedId === id) isLockedActive = saved.isLocked;
    status = next ? 'Locked' : 'Unlocked';
  };

  const copyNote = (id: number) => {
    clipboard = { id, mode: 'copy' };
    status = 'Copied';
  };

  const cutNote = (id: number) => {
    clipboard = { id, mode: 'cut' };
    status = 'Cut';
  };

  const pasteNote = async (targetParentId: number | null) => {
    if (!clipboard) return;
    const { id, mode } = clipboard;
    if (mode === 'copy') {
      const created = await notesService.duplicate(id);
      notes = [created, ...notes];
      await moveNoteTo(created.id, targetParentId);
    } else {
      const moved = await moveNoteTo(id, targetParentId);
      if (moved) clipboard = null;
    }
  };

  const deleteNoteById = async (id: number) => {
    const descendantIds = collectDescendantNoteIds(id);
    const removedIds = new Set([id, ...descendantIds]);
    const message =
      descendantIds.size > 0
        ? `Delete this note and ${descendantIds.size} note${descendantIds.size === 1 ? '' : 's'} inside it? This cannot be undone.`
        : 'Delete this note? This cannot be undone.';
    if (!(await confirmDialog(message))) return;

    await notesService.delete(id);
    notes = notes.filter((n) => !removedIds.has(n.id));
    if (selectedId != null && removedIds.has(selectedId)) {
      selectedId = null;
      if (notes.length) {
        selectNote(notes[0]);
      } else {
        title = 'Untitled';
        noteText = '';
        activeParentId = null;
      }
    }
    if (activeParentId != null && removedIds.has(activeParentId)) {
      activeParentId = null;
    }
    status = 'Deleted';
  };

  // ---------- tree state ----------

  // Deliberately doesn't touch activeParentId: expanding/collapsing a note to
  // browse its children shouldn't change where "New note" lands - that's
  // driven only by whichever note you actually have open (see selectNote).
  const toggleExpand = (id: number) => {
    const next = new Set(expandedNotes);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    expandedNotes = next;
    saveExpanded();
  };

  // ---------- context menus ----------

  const closeContextMenu = () => {
    contextMenu = null;
  };

  const confirmDialog = (message: string): Promise<boolean> => {
    return new Promise((resolve) => {
      confirmState = { message, resolve };
    });
  };

  const openBackgroundMenu = (event: MouseEvent) => {
    contextMenu = {
      x: event.clientX,
      y: event.clientY,
      items: [
        { label: 'New note', action: () => void createNoteIn(activeParentId) },
        { label: 'Paste', disabled: clipboard == null, action: () => void pasteNote(activeParentId) },
        { label: '', separator: true },
        { label: 'Refresh', action: () => void refreshAll() },
        { label: 'Collapse all', action: () => {
            expandedNotes = new Set();
            saveExpanded();
          } },
      ],
    };
  };

  const openNoteMenu = (event: MouseEvent, noteId: number) => {
    const note = notes.find((n) => n.id === noteId);
    const locked = note?.isLocked ?? false;
    contextMenu = {
      x: event.clientX,
      y: event.clientY,
      items: [
        { label: 'Open', action: () => void openNote(noteId, true) },
        { label: 'New subnote', action: () => void createNoteIn(noteId) },
        { label: 'Rename', disabled: locked, action: () => (renamingKey = `note:${noteId}`) },
        { label: 'Duplicate', action: () => void duplicateNote(noteId) },
        { label: 'Move to…', submenu: buildMoveTargetItems((target) => void moveNoteTo(noteId, target), noteId) },
        { label: '', separator: true },
        { label: locked ? 'Unlock' : 'Lock', action: () => void toggleLock(noteId) },
        { label: '', separator: true },
        { label: 'Delete', danger: true, action: () => void deleteNoteById(noteId) },
      ],
    };
  };

  const openEditorMenu = (event: MouseEvent) => {
    if (selectedId == null) return;
    const id = selectedId;
    contextMenu = {
      x: event.clientX,
      y: event.clientY,
      items: [
        { label: 'Copy', action: () => copyNote(id) },
        { label: 'Cut', action: () => cutNote(id) },
        { label: 'Paste', disabled: clipboard == null, action: () => void pasteNote(id) },
        { label: '', separator: true },
        { label: isLockedActive ? 'Unlock' : 'Lock', action: () => void toggleLock(id) },
      ],
    };
  };

  $: treeNodeProps = {
    expandedNotes,
    selectedNoteId: selectedId,
    focusedKey,
    renamingKey,
    cutId: clipboard?.mode === 'cut' ? clipboard.id : null,
    draggingId,
    dropDisabledIds,
    onToggleExpand: toggleExpand,
    onSelectNote: (id: number) => void openNote(id),
    onNoteContextMenu: openNoteMenu,
    onFocusItem: (key: string) => {
      focusedKey = key;
      treeEl?.focus();
    },
    onRenameCommit: commitRename,
    onRenameCancel: () => (renamingKey = null),
    onDragStartRow,
    onDragEndRow,
    onDropRow,
  };

  // ---------- keyboard navigation ----------

  const handleTreeKeydown = (event: KeyboardEvent) => {
    if (!visibleFlat.length) return;
    const currentIndex = visibleFlat.findIndex((v) => v.key === focusedKey);

    if (event.key === 'ArrowDown') {
      event.preventDefault();
      const next = visibleFlat[Math.min(currentIndex + 1, visibleFlat.length - 1)];
      focusedKey = next?.key ?? visibleFlat[0].key;
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      const prevIndex = currentIndex <= 0 ? 0 : currentIndex - 1;
      focusedKey = visibleFlat[prevIndex]?.key ?? visibleFlat[0].key;
    } else if (event.key === 'ArrowRight' && !isSearching) {
      const entry = visibleFlat[currentIndex];
      if (entry?.item.children.length && !expandedNotes.has(entry.item.id)) {
        event.preventDefault();
        toggleExpand(entry.item.id);
      }
    } else if (event.key === 'ArrowLeft' && !isSearching) {
      const entry = visibleFlat[currentIndex];
      if (entry?.item.children.length && expandedNotes.has(entry.item.id)) {
        event.preventDefault();
        toggleExpand(entry.item.id);
      }
    } else if (event.key === 'Enter') {
      event.preventDefault();
      if (isSearching) {
        goToSearchMatch(event.shiftKey ? -1 : 1);
        return;
      }
      const entry = visibleFlat[currentIndex];
      if (entry) {
        void openNote(entry.item.id, true);
        if (entry.item.children.length) {
          toggleExpand(entry.item.id);
        }
      }
    }
  };

  // Re-applies whichever palette is assigned to the currently active
  // light/dark mode - called on startup and any time the mode or either
  // palette assignment changes, so the visible palette always matches both.
  const applyActivePalette = () => {
    const id = theme === 'light' ? lightPaletteId : darkPaletteId;
    applyPalette(getPalette(id));
  };

  const toggleTheme = () => {
    theme = theme === 'dark' ? 'light' : 'dark';
    document.documentElement.dataset.theme = theme;
    void settingsService.saveTheme(theme);
    applyActivePalette();
  };

  const setLightPalette = (id: string) => {
    lightPaletteId = id;
    void settingsService.saveLightPalette(id);
    if (theme === 'light') applyActivePalette();
  };

  const setDarkPalette = (id: string) => {
    darkPaletteId = id;
    void settingsService.saveDarkPalette(id);
    if (theme === 'dark') applyActivePalette();
  };

  const openInsertMenu = () => {
    if (!insertButton) return;
    const rect = insertButton.getBoundingClientRect();
    contextMenu = {
      x: rect.left,
      y: rect.bottom + 4,
      items: [
        { label: 'Newline', action: () => insertNewline() },
        { label: 'Timestamp', action: () => insertTimestamp() },
        { label: 'Dateline', action: () => insertDateline() },
      ],
    };
  };

  const openNotesMenu = () => {
    if (!notesButton) return;
    const rect = notesButton.getBoundingClientRect();
    contextMenu = {
      x: rect.left,
      y: rect.bottom + 4,
      items: [
        { label: 'New note', action: () => void createNoteIn(activeParentId) },
        { label: 'New subnote', disabled: selectedId == null, action: () => {
            if (selectedId != null) void createNoteIn(selectedId);
          } },
        { label: '', separator: true },
        { label: isLockedActive ? 'Unlock' : 'Lock', disabled: selectedId == null, action: () => {
            if (selectedId != null) void toggleLock(selectedId);
          } },
        { label: '', separator: true },
        { label: 'Delete', danger: true, action: () => {
            if (selectedId != null) void deleteNoteById(selectedId);
          } },
      ],
    };
  };

  const handleKeydown = (event: KeyboardEvent) => {
    if (event.altKey && event.key === '1') {
      event.preventDefault();
      insertNewline();
    }

    if (event.altKey && event.key === '2') {
      event.preventDefault();
      insertTimestamp();
    }

    if (event.altKey && event.key === '3') {
      event.preventDefault();
      insertDateline();
    }

    if (event.altKey && event.key.toLowerCase() === 'n') {
      event.preventDefault();
      void createNoteIn(activeParentId);
    }

    if (event.altKey && event.key.toLowerCase() === 'l') {
      event.preventDefault();
      if (selectedId != null) void toggleLock(selectedId);
    }

    if (event.altKey && event.key.toLowerCase() === 'd') {
      event.preventDefault();
      if (selectedId != null) void deleteNoteById(selectedId);
    }

    if (event.key === 'Escape') {
      if (contextMenu || shortcutsOpen || settingsOpen || markdownHelpOpen || confirmState) return;
      event.preventDefault();
      void invoke('hide_window').catch(() => {
        status = 'Window hidden';
      });
    }
  };

  onMount(async () => {
    expandedNotes = loadExpanded();
    sidebarWidth = loadSidebarWidth();
    try {
      // Loaded and applied ahead of databaseService.init() below,
      // deliberately in its own try/catch: the visible theme shouldn't
      // depend on the database being reachable, and the fallback values in
      // app.css only cover the case where this never runs at all.
      const settings = await settingsService.load();
      theme = settings.theme;
      lightPaletteId = settings.lightPaletteId;
      darkPaletteId = settings.darkPaletteId;
      document.documentElement.dataset.theme = theme;
      applyActivePalette();
    } catch (err) {
      console.error('FlashPad failed to load settings', err);
    }
    try {
      await databaseService.init();
      hotkeySetting = await hotkeyService.get();

      const appState = await databaseService.getAppState();
      if (appState && !appState.ready) {
        startupError = appState.error ?? 'The configured database is unavailable.';
      } else {
        await initializeNotes();
      }
    } catch (err) {
      console.error('FlashPad failed to initialize', err);
      status = err instanceof Error ? err.message : 'Startup error';
    } finally {
      // Window starts invisible (tauri.conf.json) specifically so nothing
      // shows before this point - the double rAF waits for the browser to
      // have actually painted the just-loaded content (size/theme/notes),
      // rather than revealing a still-empty frame that then jumps to the
      // real layout. Runs even on init failure so the app isn't stuck
      // invisible if something above threw.
      requestAnimationFrame(() => requestAnimationFrame(() => void invoke('frontend_ready').catch(() => {})));
    }

    window.addEventListener('keydown', handleKeydown);
    return () => window.removeEventListener('keydown', handleKeydown);
  });
</script>

<svelte:head>
  <title>FlashPad</title>
</svelte:head>

{#if startupError}
  <div class="startup-error-shell">
    <ResizeHandles />
    <TitleBar />
    <div class="startup-error-body">
      <h2>FlashPad can't reach your database</h2>
      <p>{startupError}</p>
      <div class="startup-error-actions">
        <button class="btn" on:click={() => void retryStartup()}>Retry</button>
        <button class="btn primary" on:click={() => (settingsOpen = true)}>Open Settings</button>
      </div>
    </div>
  </div>
{:else}
<div class="app-shell">
  <ResizeHandles />
  <TitleBar />
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="action-toolbar" on:contextmenu|preventDefault>
    <button class="toolbar-btn" bind:this={notesButton} on:click={openNotesMenu} aria-label="Notes">
      <svg width="11" height="11" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
        <path d="M4 1.5h5.17a1 1 0 0 1 .7.3l2.83 2.83a1 1 0 0 1 .3.7V14a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V2.5a1 1 0 0 1 1-1Z" />
      </svg>
      <span>Notes</span>
      <svg class="caret" width="7" height="7" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
        <path d="M2.5 3.5L5 6.5L7.5 3.5" />
      </svg>
    </button>

    <button class="toolbar-btn" bind:this={insertButton} on:click={openInsertMenu} aria-label="Insert">
      <svg width="10" height="10" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
        <path d="M8 3v10M3 8h10" />
      </svg>
      <span>Insert</span>
      <svg class="caret" width="7" height="7" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
        <path d="M2.5 3.5L5 6.5L7.5 3.5" />
      </svg>
    </button>

    <div class="toolbar-right">
      {#if isMarkdownActive}
        <button class="toolbar-btn" on:click={() => (markdownHelpOpen = true)} aria-label="Markdown guide">
          <svg width="11" height="11" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
            <path d="M2 4h5M2 8h5M2 12h3" />
            <path d="M10.5 3.5v9M10.5 3.5l2 2.5 2-2.5M14.5 8.5l-2 2.5-2-2.5" />
          </svg>
          <span>Markdown</span>
        </button>
      {/if}

      <button class="toolbar-btn" on:click={() => (shortcutsOpen = true)} aria-label="Keyboard shortcuts">
        <svg width="11" height="11" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round">
          <circle cx="8" cy="8" r="6.5" />
          <path d="M6.1 6.2a1.9 1.9 0 1 1 2.7 1.7c-.7.35-.9.7-.9 1.4" stroke-linejoin="round" />
          <circle cx="8" cy="11.4" r="0.15" fill="currentColor" />
        </svg>
        <span>Shortcuts</span>
      </button>

      <button class="toolbar-btn" on:click={() => (settingsOpen = true)} aria-label="Settings">
        <svg width="11" height="11" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="8" cy="8" r="2.2" />
          <path d="M8 2v1.6M8 12.4V14M14 8h-1.6M3.6 8H2M12.13 3.87l-1.13 1.13M4.99 11.01l-1.13 1.13M12.13 12.13l-1.13-1.13M4.99 4.99 3.87 3.87" />
        </svg>
        <span>Settings</span>
      </button>
    </div>
  </div>

  <div class="shell">
  <aside class="sidebar" style="width: {sidebarWidth}px">
    <div
      class="tree"
      bind:this={treeEl}
      tabindex="0"
      role="tree"
      on:keydown={handleTreeKeydown}
      on:contextmenu|preventDefault={openBackgroundMenu}
    >
      {#if isSearching}
        {#each searchResults as note (note.id)}
          <TreeNode item={{ id: note.id, title: note.title, children: [], isMarkdown: note.isMarkdown, isLocked: note.isLocked, createdAt: note.createdAt, sortOrder: note.sortOrder }} depth={0} {...treeNodeProps} />
        {/each}
        {#if !searchResults.length}
          <p class="empty-hint">No matches</p>
        {/if}
      {:else}
        {#each tree as item (item.id)}
          <TreeNode {item} depth={0} {...treeNodeProps} />
        {/each}
        {#if !tree.length}
          <p class="empty-hint">Right-click to create a note</p>
        {/if}
        <div class="tree-spacer"></div>
      {/if}
    </div>

    <div class="sidebar-bottom">
      <button class="theme-toggle" on:click={toggleTheme} aria-label="Toggle theme">
        {#if theme === 'dark'}
          <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round">
            <circle cx="8" cy="8" r="3" />
            <path
              d="M8 1v1.5M8 13.5V15M1 8h1.5M13.5 8H15M3.2 3.2l1.1 1.1M11.7 11.7l1.1 1.1M12.8 3.2l-1.1 1.1M4.3 11.7l-1.1 1.1"
            />
          </svg>
        {:else}
          <svg width="12" height="12" viewBox="0 0 16 16" fill="currentColor">
            <path d="M13.5 9.5A6 6 0 1 1 6.5 2.5a5 5 0 1 0 7 7Z" />
          </svg>
        {/if}
      </button>
    </div>
  </aside>

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="sidebar-resizer" class:active={isResizingSidebar} on:mousedown={startSidebarResize}></div>

  <section class="editor-pane">
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <header class="topbar" on:contextmenu|preventDefault={openEditorMenu}>
      <div class="title-block">
        <input bind:value={title} class="title" placeholder="Untitled" readonly={isLockedActive} on:input={handleTitleInput} />
        <span class="breadcrumb">{currentPath}</span>
      </div>
      {#if isLockedActive}
        <svg class="icon lock-indicator" width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" aria-label="Locked">
          <rect x="3.5" y="7" width="9" height="7" rx="1.2" />
          <path d="M5.5 7V4.5a2.5 2.5 0 0 1 5 0V7" />
        </svg>
      {/if}
    </header>

    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="editor-content" on:contextmenu|preventDefault={openEditorMenu}>
      {#if isMarkdownActive}
        <MarkdownEditor
          bind:this={markdownEditorRef}
          content={noteText}
          noteId={selectedId ?? -1}
          onUpdate={handleMarkdownEditorUpdate}
          placeholder="Start typing instantly..."
          editable={!isLockedActive}
        />
      {:else}
        <textarea
          bind:this={textarea}
          bind:value={noteText}
          class="editor"
          placeholder="Start typing instantly..."
          readonly={isLockedActive}
          on:input={handleEditorInput}
        ></textarea>
      {/if}
    </div>

    <footer class="footer" on:contextmenu|preventDefault>
      <div class="search-box">
        <input class="search-input" bind:value={query} on:keydown={handleTreeKeydown} placeholder="Search notes" />
        {#if isSearching}
          <span class="search-count">{searchResults.length ? `${searchMatchIndex + 1}/${searchResults.length}` : '0/0'}</span>
          <button
            class="search-nav-btn"
            on:click={() => goToSearchMatch(-1)}
            disabled={!searchResults.length}
            aria-label="Previous match"
          >
            <svg width="8" height="8" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
              <path d="M2.5 6.5L5 3.5L7.5 6.5" />
            </svg>
          </button>
          <button
            class="search-nav-btn"
            on:click={() => goToSearchMatch(1)}
            disabled={!searchResults.length}
            aria-label="Next match"
          >
            <svg width="8" height="8" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
              <path d="M2.5 3.5L5 6.5L7.5 3.5" />
            </svg>
          </button>
        {/if}
      </div>
      <button
        class="md-toggle"
        class:active={isMarkdownActive}
        on:click={toggleMarkdown}
        disabled={selectedId == null || isLockedActive}
        aria-pressed={isMarkdownActive}
      >
        Markdown
      </button>
      <div class="status-group">
        <span class="status">{status}</span>
        {#if selectedNoteCreatedAt}
          <span class="created-hint">{selectedNoteCreatedAt.replace('T', ' ')}</span>
        {/if}
      </div>
    </footer>
  </section>
  </div>
</div>
{/if}

{#if contextMenu}
  <ContextMenu x={contextMenu.x} y={contextMenu.y} items={contextMenu.items} onClose={closeContextMenu} />
{/if}

{#if shortcutsOpen}
  <ShortcutsPanel hotkey={hotkeySetting} onClose={() => (shortcutsOpen = false)} />
{/if}

{#if settingsOpen}
  <SettingsPanel
    hotkey={hotkeySetting}
    onHotkeyChange={(next) => (hotkeySetting = next)}
    {lightPaletteId}
    {darkPaletteId}
    onLightPaletteChange={setLightPalette}
    onDarkPaletteChange={setDarkPalette}
    onClose={() => (settingsOpen = false)}
    onSwitchDatabase={switchToDatabase}
    onRequestConfirm={confirmDialog}
    onImported={async () => {
      startupError = null;
      resetNoteScopedState();
      await initializeNotes();
    }}
    onReloaded={handleDatabaseReloaded}
  />
{/if}

{#if markdownHelpOpen}
  <MarkdownHelpPanel onClose={() => (markdownHelpOpen = false)} />
{/if}

{#if confirmState}
  <ConfirmDialog
    message={confirmState.message}
    onConfirm={() => {
      confirmState?.resolve(true);
      confirmState = null;
    }}
    onCancel={() => {
      confirmState?.resolve(false);
      confirmState = null;
    }}
  />
{/if}

<style>
  /* --bg/--panel/--panel-2/--text/etc are no longer set here - the active
     palette (any of the FlashPad or Catppuccin options, not just "light" or
     "dark") is applied at runtime via applyPalette() in the script above,
     since a static stylesheet rule can't express "whichever of six palettes
     is currently selected". color-scheme (native scrollbars/form controls)
     only ever needs to follow light/dark mode though, so that alone still
     lives here, keyed off the same data-theme attribute. */
  :global(html[data-theme='light']) {
    color-scheme: light;
  }

  :global(html:not([data-theme='light'])) {
    color-scheme: dark;
  }

  :global(body.resizing-sidebar) {
    cursor: col-resize;
    user-select: none;
  }

  :global(html) {
    /* Shared by .app-shell and every overlay/modal's backdrop, so the
       transparent window margin that makes the drop shadow visible against
       the desktop never gets painted over by a full-bleed backdrop. */
    --window-shadow-margin: 1px;
  }

  .app-shell {
    position: fixed;
    inset: var(--window-shadow-margin);
    display: flex;
    flex-direction: column;
    background: var(--bg);
    color: var(--text);
    border-radius: 0.6rem;
    overflow: hidden;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.4);
  }

  .startup-error-shell {
    position: fixed;
    inset: var(--window-shadow-margin);
    display: flex;
    flex-direction: column;
    background: var(--bg);
    color: var(--text);
    border-radius: 0.6rem;
    overflow: hidden;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.4);
  }

  .startup-error-body {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.75rem;
    padding: 2rem;
    text-align: center;
  }

  .startup-error-body h2 {
    margin: 0;
    font-size: 1rem;
  }

  .startup-error-body p {
    margin: 0;
    max-width: 32rem;
    font-size: 0.85rem;
    color: var(--muted);
    line-height: 1.5;
  }

  .startup-error-actions {
    display: flex;
    gap: 0.6rem;
    margin-top: 0.5rem;
  }

  .startup-error-actions .btn {
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    background: var(--panel-2);
    color: var(--text);
    font-size: 0.82rem;
    padding: 0.4rem 0.9rem;
    cursor: pointer;
  }

  .startup-error-actions .btn:hover {
    background: var(--border);
  }

  .startup-error-actions .btn.primary {
    background: var(--accent-soft, var(--panel-2));
    font-weight: 600;
  }

  .shell {
    display: flex;
    flex: 1;
    min-height: 0;
  }

  .sidebar {
    flex-shrink: 0;
    padding: 0.75rem;
    background: var(--panel);
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    min-height: 0;
  }

  .sidebar-resizer {
    flex-shrink: 0;
    width: 5px;
    margin-left: -2px;
    margin-right: -2px;
    z-index: 10;
    cursor: col-resize;
    background: transparent;
    border-right: 1px solid var(--border);
  }

  .sidebar-resizer:hover {
    border-right: 1px solid var(--muted);
  }

  .sidebar-resizer.active {
    border-right: 2px solid var(--accent);
  }

  .sidebar-bottom {
    display: flex;
    justify-content: flex-end;
    padding-top: 0.4rem;
    border-top: 1px solid var(--border);
  }

  .theme-toggle {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 1.5rem;
    height: 1.5rem;
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    background: var(--panel-2);
    color: var(--muted);
    padding: 0;
  }

  .theme-toggle:hover {
    color: var(--text);
  }

  .tree {
    flex: 1;
    overflow: auto;
    min-height: 0;
    outline: none;
    display: flex;
    flex-direction: column;
    gap: 1px;
    margin-top: 0.3rem;
    /* Put the scrollbar on the left: flip the container to RTL (which moves
       a vertical scrollbar to the left edge), then flip every row back to
       LTR so text/content still reads normally. Also reserve the scrollbar's
       width up front so rows don't reflow/narrow the moment the list grows
       long enough to actually need it. */
    direction: rtl;
    scrollbar-gutter: stable;
  }

  /* :global() is required here - .row is rendered by the child TreeNode
     component, so a plain scoped `.tree > *` never actually reaches it
     (Svelte scopes descendant selectors to elements owned by *this*
     component only). */
  .tree > :global(*) {
    direction: ltr;
  }

  .empty-hint {
    color: var(--muted);
    font-size: 0.75rem;
    padding: 0.5rem;
    margin: 0;
  }

  .tree-spacer {
    flex: 1;
    min-height: 24px;
  }

  .editor-pane {
    flex: 1;
    display: flex;
    flex-direction: column;
  }

  .editor-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .topbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem 0.9rem;
    border-bottom: 1px solid var(--border);
    gap: 0.75rem;
  }

  .title-block {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }

  .title {
    border: 0;
    background: transparent;
    color: inherit;
    font-size: 0.95rem;
    width: 100%;
    outline: none;
  }

  .breadcrumb {
    color: var(--muted);
    font-size: 0.7rem;
  }

  .lock-indicator {
    flex-shrink: 0;
    color: var(--muted);
  }

  .md-toggle {
    flex-shrink: 0;
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    background: var(--panel-2);
    color: var(--muted);
    font-size: 0.72rem;
    padding: 0.3rem 0.55rem;
    cursor: pointer;
  }

  .md-toggle:hover:not(:disabled) {
    color: var(--text);
  }

  .md-toggle.active,
  .md-toggle.active:hover {
    background: var(--border);
    color: var(--md-color);
  }

  .md-toggle:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .action-toolbar {
    display: flex;
    align-items: center;
    flex-shrink: 0;
    gap: 0.3rem;
    height: 30px;
    padding: 0 0.5rem;
    border-bottom: 1px solid var(--border);
    background: var(--panel);
  }

  .toolbar-btn {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    height: 22px;
    border: 0;
    border-radius: 0.3rem;
    padding: 0 0.4rem;
    background: transparent;
    color: var(--muted);
    font-size: 0.8rem;
    line-height: 1;
  }

  .toolbar-btn:hover {
    background: var(--panel-2);
    color: var(--text);
  }


  .toolbar-btn .caret {
    opacity: 0.7;
  }

  .toolbar-right {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    margin-left: auto;
  }

  .editor {
    flex: 1;
    width: 100%;
    border: 0;
    resize: none;
    outline: none;
    padding: 1rem 1.1rem;
    background: transparent;
    color: inherit;
    line-height: 1.55;
  }

  .editor::placeholder {
    color: var(--muted);
    opacity: 1;
  }

  .footer {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.5rem 0.9rem;
    border-top: 1px solid var(--border);
    font-size: 0.8rem;
    color: var(--muted);
  }

  .search-box {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    flex-shrink: 0;
  }

  .footer .search-input {
    width: 200px;
    flex-shrink: 0;
    border: 1px solid var(--border);
    border-radius: 0.5rem;
    background: var(--panel-2);
    color: inherit;
    font-size: 0.8rem;
    padding: 0.35rem 0.6rem;
  }

  .search-count {
    font-size: 0.75rem;
    color: var(--muted);
    min-width: 2.5rem;
    text-align: center;
  }

  .status-group {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    margin-left: auto;
  }

  .created-hint {
    font-size: 0.72rem;
    color: var(--muted);
    white-space: nowrap;
  }

  .search-nav-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 1.4rem;
    height: 1.4rem;
    border: 1px solid var(--border);
    border-radius: 0.35rem;
    background: var(--panel-2);
    color: inherit;
    cursor: pointer;
  }

  .search-nav-btn:hover:not(:disabled) {
    background: var(--panel-3, var(--panel-2));
  }

  .search-nav-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }

</style>

<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { NotesService, type NoteRecord } from './lib/services/notesService';
  import { SettingsService, type FlashPadSettings } from './lib/services/settingsService';
  import { DEFAULT_DARK_PALETTE_ID, DEFAULT_LIGHT_PALETTE_ID, applyPalette, getPalette } from './lib/theme/palettes';
  import { HotkeyService } from './lib/services/hotkeyService';
  import { DatabaseService, type AppState } from './lib/services/databaseService';
  import TreeNode, { type TreeItem } from './lib/components/TreeNode.svelte';
  import SidebarResizer from './lib/components/SidebarResizer.svelte';
  import NoteInfoPopover from './lib/components/NoteInfoPopover.svelte';
  import ContextMenu, { type ContextMenuItem } from './lib/components/ContextMenu.svelte';
  import ShortcutsPanel from './lib/components/ShortcutsPanel.svelte';
  import SettingsPanel from './lib/components/SettingsPanel.svelte';
  import ActionToolbar from './lib/components/ActionToolbar.svelte';
  import Footer from './lib/components/Footer.svelte';
  import MarkdownEditor from './lib/components/MarkdownEditor.svelte';
  import PlainTextEditor from './lib/components/PlainTextEditor.svelte';
  import MarkdownHelpPanel from './lib/components/MarkdownHelpPanel.svelte';
  import ConfirmDialog from './lib/components/ConfirmDialog.svelte';
  import TitleBar from './lib/components/TitleBar.svelte';
  import ResizeHandles from './lib/components/ResizeHandles.svelte';
  import UpdateToast from './lib/components/UpdateToast.svelte';
  import UpdateDialog from './lib/components/UpdateDialog.svelte';
  import { check as checkForUpdate, type Update } from '@tauri-apps/plugin-updater';
  import { writeText as writeClipboardText } from '@tauri-apps/plugin-clipboard-manager';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { isAllowedLinkUrl } from './lib/utils/links';
  import {
    notes,
    selectedId,
    activeParentId,
    expandedNotes,
    focusedKey,
    renamingKey,
    draggingId,
    clipboard,
    tree,
    dropDisabledIds,
    flattenVisible,
  } from './lib/stores/notesStore';
  import * as notesStore from './lib/stores/notesStore';
  import { status } from './lib/stores/statusStore';
  import {
    databases,
    activeDatabaseId,
    startupError,
    searchAllDatabases,
    otherDatabaseNotes,
    activeDatabaseName,
  } from './lib/stores/databaseStore';
  import * as databaseStore from './lib/stores/databaseStore';

  const notesService = new NotesService();
  const settingsService = new SettingsService();
  const hotkeyService = new HotkeyService();
  const databaseService = new DatabaseService();

  // A note from the active database (no databaseId) or from another one via
  // the "search all databases" toggle (see searchableNotes below).
  type SearchableNote = NoteRecord & { databaseId?: number; databaseName?: string };

  // Each database has its own independent id sequence, so a plain
  // `note:${id}` key can collide between two different databases' notes
  // once cross-database search results are merged in - tag the key with
  // databaseId whenever it's set so Svelte's keyed each-blocks (and
  // focusedKey tracking) never conflate two different notes that happen to
  // share the same numeric id. Notes from the active database (no
  // databaseId) keep the exact same key format as before.
  const searchResultKey = (note: SearchableNote): string =>
    note.databaseId != null ? `note:${note.databaseId}:${note.id}` : `note:${note.id}`;
  let contextMenu: { x: number; y: number; items: ContextMenuItem[] } | null = null;
  let confirmState: { message: string; resolve: (value: boolean) => void } | null = null;
  let shortcutsOpen = false;
  let settingsOpen = false;
  let settingsInitialTab: 'general' | 'database' = 'general';
  let markdownHelpOpen = false;
  // Populated once, from the single startup check in onMount (never
  // polled/re-checked while running) - null means either no update was
  // found or the check hasn't resolved (or failed) yet.
  let availableUpdate: Update | null = null;
  let updateDetailsOpen = false;
  let dismissedUpdateVersion: string | null = null;
  $: showUpdateToast = availableUpdate !== null && availableUpdate.version !== dismissedUpdateVersion;
  let hotkeySetting = 'Alt+S';
  // Real default/persistence lives in SidebarResizer.svelte (bound below) -
  // this initial value is only visible for the first frame before its
  // onMount overwrites it with the saved width.
  let sidebarWidth = 260;

  let noteText = '';
  let title = 'Untitled';
  let titleAutoDerive = true;
  let query = '';
  let theme: FlashPadSettings['theme'] = 'dark';
  let lightPaletteId = DEFAULT_LIGHT_PALETTE_ID;
  let darkPaletteId = DEFAULT_DARK_PALETTE_ID;
  let isMarkdownActive = false;
  let isLockedActive = false;
  let showLineNumbersActive = false;
  let markdownEditorRef: MarkdownEditor | undefined;
  let plainEditorRef: PlainTextEditor | undefined;
  let treeEl: HTMLDivElement;
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  let toastMessage: string | null = null;
  let toastTimer: ReturnType<typeof setTimeout> | undefined;

  $: normalizedQuery = query.trim().toLowerCase();
  $: isSearching = normalizedQuery.length > 0;
  // Merges in the cached other-database notes only when the toggle is on -
  // otherDatabaseNotes already carries databaseId/databaseName (from
  // CrossDatabaseNote), which a plain NoteRecord simply doesn't have, so
  // this stays a normal instant client-side filter either way.
  $: searchableNotes = ($searchAllDatabases && $databases.length > 1 ? [...$notes, ...$otherDatabaseNotes] : $notes) as SearchableNote[];
  $: searchResults = isSearching
    ? searchableNotes
        .filter((n) => `${n.title} ${n.content}`.toLowerCase().includes(normalizedQuery))
        .sort((a, b) => a.title.localeCompare(b.title, undefined, { sensitivity: 'base' }))
    : [];
  $: visibleFlat = isSearching
    ? searchResults.map((n) => ({
        key: searchResultKey(n),
        item: { id: n.id, title: n.title, children: [], isMarkdown: n.isMarkdown, isLocked: n.isLocked, createdAt: n.createdAt, sortOrder: n.sortOrder, databaseId: n.databaseId, databaseName: n.databaseName } as TreeItem,
      }))
    : flattenVisible($tree, $expandedNotes);
  $: if (visibleFlat.length && !visibleFlat.some((v) => v.key === $focusedKey)) {
    focusedKey.set(visibleFlat[0].key);
  }
  $: searchMatchIndex = isSearching ? searchResults.findIndex((n) => n.id === $selectedId) : -1;
  $: selectedNoteCreatedAt = $notes.find((n) => n.id === $selectedId)?.createdAt ?? null;
  $: selectedNoteUpdatedAt = $notes.find((n) => n.id === $selectedId)?.updatedAt ?? null;

  // ---------- data loading ----------

  // Loads notes for whichever database is currently active and selects
  // something to show. Extracted out of onMount so switching databases (or
  // importing into the active one) can re-run exactly the same startup
  // sequence without a full app reload.
  const initializeNotes = async () => {
    await notesStore.refreshAll();
    if ($notes.length) {
      selectNote($notes[0]);
    } else {
      selectNote(await notesStore.createWelcomeNote(hotkeySetting));
    }
    requestAnimationFrame(() => (isMarkdownActive ? markdownEditorRef : plainEditorRef)?.focus());
  };

  // Resets everything scoped to the previously-active database's notes so
  // no stale ids from the old vault leak into tree-expansion, clipboard, or
  // search state after switching to a different database.
  const resetNoteScopedState = () => {
    notesStore.resetSelection();
    query = '';
  };

  // Shared by every path that can hand back a fresh AppState after
  // touching the active connection (switching, reloading, retrying startup).
  const applyAppState = async (state: AppState | null, unavailableMessage: string) => {
    if (!databaseStore.applyDatabaseState(state, unavailableMessage)) return;
    resetNoteScopedState();
    await initializeNotes();
    // Keep the cross-database cache correct relative to whichever database
    // just became active (the note just opened moves from "other" to
    // "active" and should stop showing a badge) - only worth the round
    // trip when the toggle is actually on.
    if ($searchAllDatabases) await databaseStore.refreshOtherDatabaseNotes();
  };

  // Cycles to the next database in the list (wrapping around) - lets Alt+B
  // switch databases without opening Settings first. A no-op with 0 or 1
  // databases.
  const cycleDatabase = () => {
    if ($databases.length < 2) return;
    const currentIndex = $databases.findIndex((db) => db.id === $activeDatabaseId);
    const next = $databases[(currentIndex + 1) % $databases.length];
    void switchToDatabase(next.id);
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
    selectedId.set(note.id);
    activeParentId.set(note.parentId);
    title = note.title;
    noteText = note.content;
    isMarkdownActive = note.isMarkdown;
    isLockedActive = note.isLocked;
    showLineNumbersActive = note.showLineNumbers;
    titleAutoDerive = note.title === 'Untitled' || note.title.trim() === '';
    // Plain-text undo history is per-note - PlainTextEditor resets its own
    // stacks internally when its noteId prop changes, so there's nothing to
    // reset here.
    if (focusEditor) requestAnimationFrame(() => (isMarkdownActive ? markdownEditorRef : plainEditorRef)?.focus());
  };

  // focusEditor defaults to false here: opening a note from the sidebar (click,
  // search nav) should keep keyboard focus in the tree/search box so arrow-key
  // navigation keeps working. Pass true for deliberate "open to edit" actions
  // (Enter, context menu "Open").
  const openNote = async (id: number, focusEditor = false) => {
    const note = $notes.find((n) => n.id === id);
    if (note) selectNote(note, focusEditor);
  };

  // Used for search results specifically (both click and Enter-to-cycle) -
  // a result from a different database must switch the active database
  // first (the only way to read/edit a non-active database's notes today -
  // see switchToDatabase above), then open the note normally. Query and the
  // cross-database toggle survive the switch even though
  // resetNoteScopedState (run by every other switch-database path) clears
  // the search box - that's the right default for Alt+B/manual switches,
  // just not for "I clicked a search result".
  const openSearchResult = async (id: number, databaseId?: number) => {
    if (databaseId != null && databaseId !== $activeDatabaseId) {
      // switchToDatabase -> applyAppState already refreshes
      // otherDatabaseNotes (when the toggle is on) as part of its normal
      // post-switch sequence - no need to do it again here.
      const savedQuery = query;
      await switchToDatabase(databaseId);
      query = savedQuery;
    }
    await openNote(id);
  };

  // Alt+T - toggles keyboard focus between the notes menu and the open
  // note's editor. Direction is derived from where focus actually is
  // (rather than tracked separately) so it stays correct no matter how
  // focus got there (mouse click, Tab, etc).
  const toggleMenuFocus = () => {
    const active = document.activeElement;
    if (treeEl && active && treeEl.contains(active)) {
      if ($selectedId == null) return;
      if (isMarkdownActive) {
        markdownEditorRef?.focus();
      } else {
        plainEditorRef?.focus();
      }
      return;
    }
    // Focus the row for whichever note is currently open (falls back to
    // the first visible row automatically - see the visibleFlat/focusedKey
    // sync above).
    if ($selectedId != null) focusedKey.set(`note:${$selectedId}`);
    treeEl?.focus();
  };

  const goToSearchMatch = (direction: 1 | -1) => {
    if (!searchResults.length) return;
    const nextIndex = searchMatchIndex === -1
      ? 0
      : (searchMatchIndex + direction + searchResults.length) % searchResults.length;
    const match = searchResults[nextIndex];
    focusedKey.set(searchResultKey(match));
    void openSearchResult(match.id, match.databaseId);
  };

  const saveActiveNote = async () => {
    if (!$selectedId) return;
    // Locked notes only ever reach here via a checkbox toggle (see
    // onReadOnlyChecked in MarkdownEditor.svelte) - route through the
    // narrow exception that persists just the content, rather than the
    // general save, which rejects content changes on a locked note.
    const saved = isLockedActive
      ? await notesService.saveChecklistToggle($selectedId, noteText)
      : await notesService.save({ id: $selectedId, title, content: noteText });
    notesStore.applyUpdatedNote(saved);
    status.set('Saved');
  };

  const scheduleSave = () => {
    status.set('Saving…');
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      void saveActiveNote();
    }, 250);
  };

  // Not guarded by isLockedActive: every OTHER path into this (typing in
  // the plain textarea, insertAtCursor, plain-text undo/redo) is already
  // blocked upstream while locked (native readonly / explicit checks), so
  // in practice this only ever runs locked via the Markdown editor's
  // checkbox-toggle exception below - which is exactly the one edit a
  // locked note should still save.
  const handleEditorInput = () => {
    if (titleAutoDerive) {
      title = deriveTitleFromContent(noteText, isMarkdownActive);
    }
    scheduleSave();
  };

  // Also fires for a checkbox toggle in a locked note (see
  // onReadOnlyChecked in MarkdownEditor.svelte) - a locked note's text is
  // read-only, but ticking a finished checklist's items is exactly the
  // kind of edit locking is meant to still allow, and it should save and
  // bump updated_at the same as any other edit.
  const handleMarkdownEditorUpdate = (markdown: string) => {
    noteText = markdown;
    handleEditorInput();
  };

  const handlePlainEditorUpdate = (text: string) => {
    noteText = text;
    handleEditorInput();
  };

  const handleTitleInput = () => {
    if (isLockedActive) return;
    titleAutoDerive = false;
    scheduleSave();
  };

  const toggleMarkdown = () => {
    if ($selectedId == null || isLockedActive) return;
    const next = !isMarkdownActive;
    isMarkdownActive = next;
    // Switching modes swaps the textarea/MarkdownEditor DOM out from under
    // whichever one was focused - wait for that swap to render, then focus
    // whichever editor is now showing so typing can continue immediately.
    void tick().then(() => {
      if (next) {
        markdownEditorRef?.focus();
      } else {
        plainEditorRef?.focus();
      }
    });
    void notesService.save({ id: $selectedId, isMarkdown: next }).then((saved) => {
      notesStore.applyUpdatedNote(saved);
    });
  };

  const insertAtCursor = (text: string) => {
    if (isLockedActive) return;
    if (isMarkdownActive) {
      markdownEditorRef?.insertAtCursor(text);
    } else {
      plainEditorRef?.insertAtCursor(text);
    }
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
    selectNote(await notesStore.createNoteIn(parentId));
  };

  // Triggered from Settings, which shows its own inline "Importing…"/result
  // state (same pattern as the "Check for updates" button) rather than this
  // reporting through the main window's status bar - errors are left to
  // propagate so Settings can display them. Returns null if the user
  // cancelled the folder picker, distinct from a real failure.
  const importFromFolder = async (): Promise<{ importedCount: number } | null> => {
    const result = await notesStore.importFromFolder();
    if (!result) return null;
    if (result.imported) selectNote(result.imported);
    return { importedCount: result.importedCount };
  };

  // ---------- rename / move / delete / duplicate ----------

  const commitRename = async (key: string, value: string) => {
    const result = await notesStore.commitRename(key, value);
    if (result && $selectedId === result.id) {
      title = result.updated.title;
      titleAutoDerive = false;
    }
  };

  const duplicateNote = async (id: number) => {
    selectNote(await notesStore.duplicateNote(id));
  };

  const toggleLock = async (id: number) => {
    const saved = await notesStore.toggleLock(id);
    if (saved && $selectedId === id) isLockedActive = saved.isLocked;
  };

  const showToast = (message: string) => {
    toastMessage = message;
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toastMessage = null), 3500);
  };

  // Re-checked here even though the Link extension in MarkdownEditor.svelte
  // already restricts which hrefs can become a link mark in the first
  // place - note content can arrive from a paste (a crafted anchor from a
  // webpage), so never trust the frontend DOM alone for something that
  // reaches out to the OS.
  const openLink = async (url: string) => {
    if (!isAllowedLinkUrl(url)) {
      showToast("Can't open this link");
      return;
    }
    try {
      await openUrl(url);
    } catch (err) {
      showToast(err instanceof Error ? err.message : 'Failed to open link');
    }
  };

  const deleteNoteById = async (id: number) => {
    const descendantIds = notesStore.collectDescendantNoteIds(id);
    const message =
      descendantIds.size > 0
        ? `Delete this note and ${descendantIds.size} note${descendantIds.size === 1 ? '' : 's'} inside it? This cannot be undone.`
        : 'Delete this note? This cannot be undone.';
    if (!(await confirmDialog(message))) return;

    const result = await notesStore.deleteNote(id);
    if (result.removedSelected) {
      if (result.nextNote) {
        selectNote(result.nextNote);
      } else {
        title = 'Untitled';
        noteText = '';
      }
    }
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
        { label: 'New note', action: () => void createNoteIn($activeParentId) },
        { label: 'Paste', disabled: $clipboard == null, action: () => void notesStore.pasteNote($activeParentId) },
        { label: '', separator: true },
        { label: 'Refresh', action: () => void notesStore.refreshAll() },
        { label: 'Collapse all', action: () => notesStore.collapseAll() },
      ],
    };
  };

  const openNoteMenu = (event: MouseEvent, noteId: number) => {
    const note = $notes.find((n) => n.id === noteId);
    const locked = note?.isLocked ?? false;
    contextMenu = {
      x: event.clientX,
      y: event.clientY,
      items: [
        { label: 'Open', action: () => void openNote(noteId, true) },
        { label: 'New subnote', action: () => void createNoteIn(noteId) },
        { label: 'Rename', disabled: locked, action: () => renamingKey.set(`note:${noteId}`) },
        { label: 'Duplicate', action: () => void duplicateNote(noteId) },
        { label: 'Move to…', submenu: notesStore.buildMoveTargetItems((target) => void notesStore.moveNoteTo(noteId, target), noteId) },
        { label: '', separator: true },
        { label: locked ? 'Unlock' : 'Lock', action: () => void toggleLock(noteId) },
        { label: '', separator: true },
        { label: 'Delete', danger: true, action: () => void deleteNoteById(noteId) },
      ],
    };
  };

  const openEditorMenu = (event: MouseEvent) => {
    if ($selectedId == null) return;
    const id = $selectedId;
    // Links only exist in Markdown mode - closest('a[href]') naturally
    // finds nothing in the plain textarea, but the isMarkdownActive check
    // is kept as the source of truth rather than relying on that.
    const linkHref = isMarkdownActive
      ? ((event.target as HTMLElement | null)?.closest?.('a[href]') as HTMLAnchorElement | null)?.getAttribute('href') ?? null
      : null;
    contextMenu = {
      x: event.clientX,
      y: event.clientY,
      items: [
        ...(linkHref
          ? [
              { label: 'Open link', action: () => void openLink(linkHref) },
              { label: 'Copy link address', action: () => void writeClipboardText(linkHref) },
              { label: '', separator: true },
            ]
          : []),
        { label: 'Copy', action: () => notesStore.copyNote(id) },
        { label: 'Cut', action: () => notesStore.cutNote(id) },
        { label: 'Paste', disabled: $clipboard == null, action: () => void notesStore.pasteNote(id) },
        { label: '', separator: true },
        { label: isLockedActive ? 'Unlock' : 'Lock', action: () => void toggleLock(id) },
      ],
    };
  };

  $: treeNodeProps = {
    expandedNotes: $expandedNotes,
    selectedNoteId: $selectedId,
    focusedKey: $focusedKey,
    renamingKey: $renamingKey,
    cutId: $clipboard?.mode === 'cut' ? $clipboard.id : null,
    draggingId: $draggingId,
    dropDisabledIds: $dropDisabledIds,
    onToggleExpand: notesStore.toggleExpand,
    onSelectNote: (id: number, databaseId?: number) => void openSearchResult(id, databaseId),
    onNoteContextMenu: openNoteMenu,
    onFocusItem: (key: string) => {
      focusedKey.set(key);
      treeEl?.focus();
    },
    onRenameCommit: commitRename,
    onRenameCancel: () => renamingKey.set(null),
    onDragStartRow: notesStore.onDragStartRow,
    onDragEndRow: notesStore.onDragEndRow,
    onDropRow: notesStore.onDropRow,
  };

  // ---------- keyboard navigation ----------

  const handleTreeKeydown = (event: KeyboardEvent) => {
    if (!visibleFlat.length) return;
    const currentIndex = visibleFlat.findIndex((v) => v.key === $focusedKey);

    if (event.key === 'ArrowDown') {
      event.preventDefault();
      const next = visibleFlat[Math.min(currentIndex + 1, visibleFlat.length - 1)];
      focusedKey.set(next?.key ?? visibleFlat[0].key);
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      const prevIndex = currentIndex <= 0 ? 0 : currentIndex - 1;
      focusedKey.set(visibleFlat[prevIndex]?.key ?? visibleFlat[0].key);
    } else if (event.key === 'ArrowRight' && !isSearching) {
      const entry = visibleFlat[currentIndex];
      if (entry?.item.children.length && !$expandedNotes.has(entry.item.id)) {
        event.preventDefault();
        notesStore.toggleExpand(entry.item.id);
      }
    } else if (event.key === 'ArrowLeft' && !isSearching) {
      const entry = visibleFlat[currentIndex];
      if (entry?.item.children.length && $expandedNotes.has(entry.item.id)) {
        event.preventDefault();
        notesStore.toggleExpand(entry.item.id);
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
          notesStore.toggleExpand(entry.item.id);
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

  // Per-note, toggled via Alt+R - not gated on isLockedActive, since this is
  // a display preference rather than an edit to the note's protected text
  // (and update_note's lock guard only rejects title/content changes
  // anyway, so this always goes through even on a locked note).
  const toggleLineNumbers = async (id: number) => {
    const saved = await notesStore.toggleLineNumbers(id);
    if (saved && $selectedId === id) showLineNumbersActive = saved.showLineNumbers;
  };

  // Checked once on startup only (called from onMount, never polled/re-run
  // while the app is open) - failures (no internet, GitHub unreachable,
  // etc.) are swallowed silently since a missed check just means no
  // indicator shows, never anything that blocks using the app.
  const checkForAppUpdate = async () => {
    try {
      const update = await checkForUpdate();
      if (update) availableUpdate = update;
    } catch (err) {
      console.error('Update check failed', err);
    }
  };

  const dismissUpdate = () => {
    if (!availableUpdate) return;
    dismissedUpdateVersion = availableUpdate.version;
    void settingsService.saveDismissedUpdateVersion(availableUpdate.version);
    updateDetailsOpen = false;
  };

  // Manual "Check for updates" from Settings - unlike the silent startup
  // check, errors are left to propagate so Settings can show them, and the
  // dialog (with the changelog) opens immediately on top of Settings if
  // something is found, rather than waiting to be clicked from a toast.
  const checkForUpdateManually = async (): Promise<Update | null> => {
    const update = await checkForUpdate();
    if (update) {
      availableUpdate = update;
      updateDetailsOpen = true;
    }
    return update;
  };

  const openInsertMenu = (rect: DOMRect) => {
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

  const openNotesMenu = (rect: DOMRect) => {
    contextMenu = {
      x: rect.left,
      y: rect.bottom + 4,
      items: [
        { label: 'New note', action: () => void createNoteIn($activeParentId) },
        { label: 'New subnote', disabled: $selectedId == null, action: () => {
            if ($selectedId != null) void createNoteIn($selectedId);
          } },
        { label: '', separator: true },
        { label: isLockedActive ? 'Unlock' : 'Lock', disabled: $selectedId == null, action: () => {
            if ($selectedId != null) void toggleLock($selectedId);
          } },
        { label: '', separator: true },
        { label: 'Delete', danger: true, action: () => {
            if ($selectedId != null) void deleteNoteById($selectedId);
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
      void createNoteIn($activeParentId);
    }

    if (event.altKey && event.key.toLowerCase() === 'l') {
      event.preventDefault();
      if ($selectedId != null) void toggleLock($selectedId);
    }

    if (event.altKey && event.key.toLowerCase() === 'd') {
      event.preventDefault();
      if ($selectedId != null) void deleteNoteById($selectedId);
    }

    if (event.altKey && event.key.toLowerCase() === 'm') {
      event.preventDefault();
      toggleMarkdown();
    }

    if (event.altKey && event.key.toLowerCase() === 'b') {
      event.preventDefault();
      cycleDatabase();
    }

    if (event.altKey && event.key.toLowerCase() === 't') {
      event.preventDefault();
      toggleMenuFocus();
    }

    if (event.altKey && event.key.toLowerCase() === 'o') {
      event.preventDefault();
      if (isMarkdownActive) {
        const href = markdownEditorRef?.getLinkHrefAtCursor();
        if (href) void openLink(href);
      }
    }

    if (event.altKey && event.key.toLowerCase() === 'r') {
      event.preventDefault();
      if ($selectedId != null) void toggleLineNumbers($selectedId);
    }

    if (event.key === 'Escape') {
      if (contextMenu || shortcutsOpen || settingsOpen || markdownHelpOpen || confirmState || updateDetailsOpen) return;
      event.preventDefault();
      void invoke('hide_window').catch(() => {
        status.set('Window hidden');
      });
    }
  };

  onMount(async () => {
    try {
      // Loaded and applied ahead of databaseService.init() below,
      // deliberately in its own try/catch: the visible theme shouldn't
      // depend on the database being reachable, and the fallback values in
      // app.css only cover the case where this never runs at all.
      const settings = await settingsService.load();
      theme = settings.theme;
      lightPaletteId = settings.lightPaletteId;
      darkPaletteId = settings.darkPaletteId;
      dismissedUpdateVersion = settings.dismissedUpdateVersion;
      document.documentElement.dataset.theme = theme;
      applyActivePalette();
    } catch (err) {
      console.error('FlashPad failed to load settings', err);
    }
    try {
      await databaseService.init();
      hotkeySetting = await hotkeyService.get();

      // appState is only ever null outside Tauri (the browser-preview
      // fallback, which has no database concept at all) - deliberately NOT
      // routed through applyDatabaseState, which would treat null as an
      // unreachable-database failure. Every other real caller
      // (switchToDatabase, retryStartup, handleDatabaseReloaded) already
      // goes through applyAppState/applyDatabaseState instead, where a null
      // state is a genuine failure.
      const appState = await databaseService.getAppState();
      if (appState) {
        databases.set(appState.databases);
        activeDatabaseId.set(appState.activeDatabaseId);
      }
      if (appState && !appState.ready) {
        startupError.set(appState.error ?? 'The configured database is unavailable.');
      } else {
        await initializeNotes();
      }
    } catch (err) {
      console.error('FlashPad failed to initialize', err);
      status.set(err instanceof Error ? err.message : 'Startup error');
    } finally {
      // Window starts invisible (tauri.conf.json) specifically so nothing
      // shows before this point - the double rAF waits for the browser to
      // have actually painted the just-loaded content (size/theme/notes),
      // rather than revealing a still-empty frame that then jumps to the
      // real layout. Runs even on init failure so the app isn't stuck
      // invisible if something above threw.
      requestAnimationFrame(() => requestAnimationFrame(() => void invoke('frontend_ready').catch(() => {})));
    }

    void checkForAppUpdate();

    window.addEventListener('keydown', handleKeydown);
    return () => {
      window.removeEventListener('keydown', handleKeydown);
    };
  });
</script>

<svelte:head>
  <title>FlashPad</title>
</svelte:head>

{#if $startupError}
  <div class="startup-error-shell">
    <ResizeHandles />
    <TitleBar />
    <div class="startup-error-body">
      <h2>FlashPad can't reach your database</h2>
      <p>{$startupError}</p>
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
  <ActionToolbar
    {isMarkdownActive}
    onOpenNotesMenu={openNotesMenu}
    onOpenInsertMenu={openInsertMenu}
    onShowMarkdownHelp={() => (markdownHelpOpen = true)}
    onShowShortcuts={() => (shortcutsOpen = true)}
    onShowSettings={() => (settingsOpen = true)}
  />

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
        {#each searchResults as note (searchResultKey(note))}
          <TreeNode item={{ id: note.id, title: note.title, children: [], isMarkdown: note.isMarkdown, isLocked: note.isLocked, createdAt: note.createdAt, sortOrder: note.sortOrder, databaseId: note.databaseId, databaseName: note.databaseName }} depth={0} {...treeNodeProps} />
        {/each}
        {#if !searchResults.length}
          <p class="empty-hint">No matches</p>
        {/if}
      {:else}
        {#each $tree as item (item.id)}
          <TreeNode {item} depth={0} {...treeNodeProps} />
        {/each}
        {#if !$tree.length}
          <p class="empty-hint">Right-click to create a note</p>
        {/if}
        <div class="tree-spacer"></div>
      {/if}
    </div>

    <div class="sidebar-bottom">
      {#if $activeDatabaseName}
        <button
          class="db-indicator"
          type="button"
          on:click={() => {
            settingsInitialTab = 'database';
            settingsOpen = true;
          }}
          aria-label="Current database - open database settings"
          title="Switch databases with Alt+B"
        >
          <svg width="11" height="11" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round">
            <ellipse cx="8" cy="3.5" rx="5.5" ry="2" />
            <path d="M2.5 3.5V8c0 1.1 2.46 2 5.5 2s5.5-.9 5.5-2V3.5" />
            <path d="M2.5 8v4.5c0 1.1 2.46 2 5.5 2s5.5-.9 5.5-2V8" />
          </svg>
          <span>{$activeDatabaseName}</span>
        </button>
      {/if}
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

  <SidebarResizer bind:width={sidebarWidth} />

  <section class="editor-pane">
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <header class="topbar" on:contextmenu|preventDefault={openEditorMenu}>
      <div class="title-block">
        <input
          bind:value={title}
          class="title"
          placeholder="Untitled"
          readonly={isLockedActive}
          on:input={handleTitleInput}
        />
      </div>
      <div class="header-meta">
        {#if $selectedId != null}
          <NoteInfoPopover createdAt={selectedNoteCreatedAt} updatedAt={selectedNoteUpdatedAt} onError={(msg) => status.set(msg)} />
        {/if}
        {#if isLockedActive}
          <svg class="icon lock-indicator" width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" aria-label="Locked">
            <rect x="3.5" y="7" width="9" height="7" rx="1.2" />
            <path d="M5.5 7V4.5a2.5 2.5 0 0 1 5 0V7" />
          </svg>
        {/if}
      </div>
    </header>

    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="editor-content" on:contextmenu|preventDefault={openEditorMenu}>
      {#if isMarkdownActive}
        <MarkdownEditor
          bind:this={markdownEditorRef}
          content={noteText}
          noteId={$selectedId ?? -1}
          onUpdate={handleMarkdownEditorUpdate}
          onOpenLink={openLink}
          placeholder="Start typing instantly..."
          editable={!isLockedActive}
        />
      {:else}
        <PlainTextEditor
          bind:this={plainEditorRef}
          content={noteText}
          noteId={$selectedId ?? -1}
          onUpdate={handlePlainEditorUpdate}
          placeholder="Start typing instantly..."
          editable={!isLockedActive}
          showLineNumbers={showLineNumbersActive}
        />
      {/if}
    </div>

    <Footer
      bind:query
      {isSearching}
      searchResultsCount={searchResults.length}
      {searchMatchIndex}
      {isMarkdownActive}
      {isLockedActive}
      onSearchKeydown={handleTreeKeydown}
      onGoToSearchMatch={goToSearchMatch}
      onToggleMarkdown={toggleMarkdown}
    />
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
    onCheckForUpdate={checkForUpdateManually}
    onImportFromFolder={importFromFolder}
    onClose={() => {
      settingsOpen = false;
      settingsInitialTab = 'general';
      // Settings can add/rename/remove databases without going through
      // switchToDatabase - refresh so Alt+B cycling stays in sync.
      void databaseService.getAppState().then((state) => {
        if (state) {
          databases.set(state.databases);
          activeDatabaseId.set(state.activeDatabaseId);
        }
      });
    }}
    initialTab={settingsInitialTab}
    onSwitchDatabase={switchToDatabase}
    onRequestConfirm={confirmDialog}
    onImported={async () => {
      startupError.set(null);
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

{#if showUpdateToast && !updateDetailsOpen}
  <UpdateToast
    version={availableUpdate?.version ?? ''}
    onViewDetails={() => (updateDetailsOpen = true)}
    onDismiss={dismissUpdate}
  />
{/if}

{#if updateDetailsOpen && availableUpdate}
  <UpdateDialog update={availableUpdate} onDismiss={dismissUpdate} />
{/if}

{#if toastMessage}
  <div class="link-toast" role="status">{toastMessage}</div>
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

  .sidebar-bottom {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.4rem;
    padding-top: 0.4rem;
    border-top: 1px solid var(--border);
  }

  .db-indicator {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    min-width: 0;
    border: 0;
    border-radius: 0.3rem;
    padding: 0.15rem 0.3rem;
    background: transparent;
    color: var(--muted);
    font-size: 0.72rem;
  }

  .db-indicator:hover {
    background: var(--panel-2);
    color: var(--text);
  }

  .db-indicator svg {
    flex-shrink: 0;
  }

  .db-indicator span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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

  .header-meta {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-shrink: 0;
  }

  .lock-indicator {
    flex-shrink: 0;
    color: var(--muted);
  }

  .link-toast {
    position: fixed;
    left: 50%;
    bottom: calc(var(--window-shadow-margin, 0px) + 0.75rem);
    transform: translateX(-50%);
    z-index: 1300;
    max-width: min(320px, calc(100vw - 1.5rem));
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 0.6rem;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.4);
    padding: 0.5rem 0.75rem;
    font-size: 0.78rem;
    color: var(--text);
  }

</style>

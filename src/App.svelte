<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import type { NoteRecord } from './lib/services/notesService';
  import { SettingsService, type FlashPadSettings } from './lib/services/settingsService';
  import { DEFAULT_DARK_PALETTE_ID, DEFAULT_LIGHT_PALETTE_ID, applyPalette, getPalette } from './lib/theme/palettes';
  import { HotkeyService } from './lib/services/hotkeyService';
  import { DatabaseService, type AppState } from './lib/services/databaseService';
  import { BackupService } from './lib/services/backupService';
  import TreeNode, { type TreeItem } from './lib/components/TreeNode.svelte';
  import SidebarResizer from './lib/components/SidebarResizer.svelte';
  import PaneResizer from './lib/components/PaneResizer.svelte';
  import NotePane from './lib/components/NotePane.svelte';
  import ContextMenu, { type ContextMenuItem } from './lib/components/ContextMenu.svelte';
  import ShortcutsPanel from './lib/components/ShortcutsPanel.svelte';
  import SettingsPanel from './lib/components/SettingsPanel.svelte';
  import ActionToolbar from './lib/components/ActionToolbar.svelte';
  import Footer from './lib/components/Footer.svelte';
  import MarkdownHelpPanel from './lib/components/MarkdownHelpPanel.svelte';
  import ConfirmDialog from './lib/components/ConfirmDialog.svelte';
  import TitleBar from './lib/components/TitleBar.svelte';
  import ResizeHandles from './lib/components/ResizeHandles.svelte';
  import UpdateToast from './lib/components/UpdateToast.svelte';
  import UpdateDialog from './lib/components/UpdateDialog.svelte';
  import PluginFormDialog from './lib/components/PluginFormDialog.svelte';
  import PluginMessageDialog from './lib/components/PluginMessageDialog.svelte';
  import { activePluginForm, activePluginMessage } from './lib/plugins/pluginApi';
  import { ensurePluginsDirExists, loadEnabledPlugins } from './lib/plugins/pluginLoader';
  import { check as checkForUpdate, type Update } from '@tauri-apps/plugin-updater';
  import { save as saveFileDialog } from '@tauri-apps/plugin-dialog';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { isAllowedLinkUrl } from './lib/utils/links';
  import { toCrlfNewlines, prefersCrlfClipboard } from './lib/utils/clipboard';
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
    activeCloudNotebookId,
    startupError,
    searchAllDatabases,
    hasSearchableOtherSources,
    otherDatabaseNotes,
    activeDatabaseName,
  } from './lib/stores/databaseStore';
  import * as databaseStore from './lib/stores/databaseStore';
  import { authReadyPromise, session, signOut } from './lib/stores/authStore';
  import * as cloudStore from './lib/stores/cloudStore';
  import { notebooks as cloudNotebooks } from './lib/stores/cloudStore';
  import { CloudNotesService } from './lib/services/cloudNotesService';
  import { loadActiveSource, saveActiveSource, clearActiveSource, type ActiveSource } from './lib/utils/activeSource';

  const settingsService = new SettingsService();
  const hotkeyService = new HotkeyService();
  const databaseService = new DatabaseService();
  const backupService = new BackupService();

  // A note from the active database (no databaseId) or from another one via
  // the "search all databases" toggle (see searchableNotes below). number
  // for a local profile, string (a notebook uuid) for a cloud notebook.
  type SearchableNote = NoteRecord & { databaseId?: number | string; databaseName?: string };

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

  let query = '';
  let theme: FlashPadSettings['theme'] = 'dark';
  let lightPaletteId = DEFAULT_LIGHT_PALETTE_ID;
  let darkPaletteId = DEFAULT_DARK_PALETTE_ID;
  // Owned by NotePane, bound back up here since ActionToolbar/Footer need
  // to reflect them live (which buttons show) without waiting on a save
  // round-trip - see NotePane.svelte's own comment on these three props.
  // Split view means two independent copies (one per pane) - isMarkdownActive
  // etc. below are then derived from whichever pane is currently focused,
  // since that's what ActionToolbar/Footer should actually reflect.
  let primaryIsMarkdownActive = false;
  let primaryIsLockedActive = false;
  let primaryIsEditorModeActive = false;
  let secondaryIsMarkdownActive = false;
  let secondaryIsLockedActive = false;
  let secondaryIsEditorModeActive = false;
  let vimModeEnabled = false;
  let dateTimeNoteNamesEnabled = true;
  let enabledPluginIds: string[] = [];
  let primaryPaneRef: NotePane | undefined;
  let secondaryPaneRef: NotePane | undefined;
  // Split view state. splitNoteId is the SECONDARY pane's note - the
  // primary pane always shows $selectedId, exactly as it did before split
  // view existed, so every pre-existing selectedId-based reference
  // elsewhere in the app (TreeNode highlighting via focusedNoteId below,
  // search, etc.) keeps working unchanged when split view is off. Not
  // persisted across restarts - relaunching returns to single-pane.
  let splitViewEnabled = false;
  let splitNoteId: number | null = null;
  let splitRatio = 0.5;
  let focusedPane: 'primary' | 'secondary' = 'primary';

  $: isMarkdownActive = focusedPane === 'secondary' ? secondaryIsMarkdownActive : primaryIsMarkdownActive;
  $: isLockedActive = focusedPane === 'secondary' ? secondaryIsLockedActive : primaryIsLockedActive;
  $: isEditorModeActive = focusedPane === 'secondary' ? secondaryIsEditorModeActive : primaryIsEditorModeActive;
  // Whichever note the currently-focused pane is showing - Alt+L/Alt+D/
  // Alt+R and sidebar highlighting all act on this, not always $selectedId,
  // once a second pane can be focused instead.
  $: focusedNoteId = focusedPane === 'secondary' ? splitNoteId : $selectedId;

  // Dispatch point for "whichever pane currently has focus" - every
  // keyboard shortcut/toolbar action that acts on "the current note" goes
  // through this rather than hardcoding primaryPaneRef, so split view just
  // works for all of them once a second pane exists.
  const activePaneRef = () => (focusedPane === 'secondary' ? secondaryPaneRef : primaryPaneRef);

  // Clicking inside a pane (see NotePane's onFocus prop, fired on
  // mousedown in the capture phase - before a wiki-link/backlink click
  // inside it resolves, so those correctly land in whichever pane they
  // were clicked from) marks it focused. Doesn't touch $selectedId/
  // splitNoteId themselves - the panes stay showing whatever they're
  // showing, only which one shortcuts/sidebar-clicks target changes.
  const focusPane = (pane: 'primary' | 'secondary') => {
    focusedPane = pane;
  };

  const toggleSplitView = () => {
    if (splitViewEnabled) {
      splitViewEnabled = false;
      focusedPane = 'primary';
      return;
    }
    splitViewEnabled = true;
    // Starts empty rather than duplicating the primary's note - the
    // secondary pane shows its own "pick a note" placeholder (see
    // NotePane.svelte) until you click one in the sidebar while it has
    // focus, which is what focusing it here (instead of leaving the
    // primary focused) is for.
    splitNoteId = null;
    focusedPane = 'secondary';
  };

  let treeEl: HTMLDivElement;
  let toastMessage: string | null = null;
  let toastTimer: ReturnType<typeof setTimeout> | undefined;

  $: normalizedQuery = query.trim().toLowerCase();
  $: isSearching = normalizedQuery.length > 0;
  // Merges in the cached other-database notes only when the toggle is on -
  // otherDatabaseNotes already carries databaseId/databaseName (from
  // CrossDatabaseNote), which a plain NoteRecord simply doesn't have, so
  // this stays a normal instant client-side filter either way.
  $: searchableNotes = ($searchAllDatabases && $hasSearchableOtherSources ? [...$notes, ...$otherDatabaseNotes] : $notes) as SearchableNote[];
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
    requestAnimationFrame(() => activePaneRef()?.focus());
  };

  // Resets everything scoped to the previously-active database's notes so
  // no stale ids from the old vault leak into tree-expansion, clipboard,
  // search, or split-view state after switching to a different database -
  // splitNoteId in particular would otherwise keep pointing at an id from
  // a database that's no longer even loaded.
  const resetNoteScopedState = () => {
    notesStore.resetSelection();
    query = '';
    splitViewEnabled = false;
    splitNoteId = null;
    focusedPane = 'primary';
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
  // switch databases without opening Settings first. Local profiles and
  // cloud notebooks share one combined cycle order (local first, then
  // cloud) - a no-op with fewer than 2 entries total. Re-fetches the local
  // list fresh every time (a cheap local IPC call, not a network round
  // trip) rather than trusting the possibly-stale `databases` store -
  // DatabaseManagerSection maintains its own separate list while Settings
  // is open and only syncs back into this store when Settings closes, so
  // without this a database added mid-session wouldn't show up in the
  // cycle order until Settings had been closed once. cloudNotebooks only
  // reflects reality once cloudStore.refreshNotebooks() has run (App.svelte
  // does this at boot when signed in, and CloudNotebooksSection does it
  // whenever it's open), same caveat as hasSearchableOtherSources.
  type CycleEntry = { kind: 'local'; id: number } | { kind: 'cloud'; id: string };
  const cycleDatabase = async () => {
    const freshLocal = await databaseService.listDatabases();
    databases.set(freshLocal);
    const combined: CycleEntry[] = [
      ...freshLocal.map((db): CycleEntry => ({ kind: 'local', id: db.id })),
      ...$cloudNotebooks.map((nb): CycleEntry => ({ kind: 'cloud', id: nb.id })),
    ];
    if (combined.length < 2) return;
    const currentIndex = combined.findIndex((entry) =>
      $activeCloudNotebookId != null ? entry.kind === 'cloud' && entry.id === $activeCloudNotebookId : entry.kind === 'local' && entry.id === $activeDatabaseId,
    );
    const next = combined[(currentIndex + 1) % combined.length];
    if (next.kind === 'local') void switchToDatabase(next.id);
    else void switchToCloudNotebook(next.id);
  };

  const switchToDatabase = async (id: number) => {
    notesStore.useLocalNotesBackend();
    activeCloudNotebookId.set(null);
    saveActiveSource({ kind: 'local', id });
    const state = await databaseService.switchDatabase(id);
    await applyAppState(state, 'The selected database is unavailable.');
  };

  // Falls back to whichever local database is configured, used whenever a
  // persisted or just-selected cloud notebook turns out to be unreachable
  // (offline, membership revoked, notebook deleted) or the user isn't
  // signed in - a non-blocking toast instead of the full-screen
  // startupError banner, since local notes are still perfectly usable; only
  // an actually-unreachable LOCAL database warrants that banner.
  const fallBackToLocal = async (message: string) => {
    clearActiveSource();
    activeCloudNotebookId.set(null);
    notesStore.useLocalNotesBackend();
    const state = await databaseService.getAppState();
    await applyAppState(state, 'The configured database is unavailable.');
    showToast(message);
  };

  // Mirrors switchToDatabase for a cloud notebook instead of a local
  // profile - see lib/utils/activeSource.ts for why the persisted selection
  // is a tagged union of the two. A failed refreshNotes() (offline, RLS
  // denies because membership was revoked, notebook deleted) falls back to
  // local rather than leaving the UI stuck on a dead cloud notebook.
  const switchToCloudNotebook = async (notebookId: string) => {
    notesStore.setActiveNotesBackend(new CloudNotesService(notebookId));
    activeCloudNotebookId.set(notebookId);
    saveActiveSource({ kind: 'cloud', notebookId });
    resetNoteScopedState();
    try {
      startupError.set(null);
      await initializeNotes();
    } catch (err) {
      await fallBackToLocal(err instanceof Error ? err.message : 'That notebook is no longer available - showing local notes.');
      return;
    }
    if ($searchAllDatabases) await databaseStore.refreshOtherDatabaseNotes();
  };

  const handleDatabaseReloaded = async (state: AppState) => {
    await applyAppState(state, 'The database is unavailable.');
  };

  const retryStartup = async () => {
    const state = await databaseService.getAppState();
    await applyAppState(state, 'The configured database is unavailable.');
  };

  // Switches back to local (if a cloud notebook was active) before actually
  // signing out, so the UI never briefly shows cloud notes against a dead
  // session, then clears every cloud-scoped store.
  const handleSignOut = async () => {
    if ($activeCloudNotebookId != null) {
      const local = $databases.find((db) => db.id === $activeDatabaseId) ?? $databases[0];
      if (local) await switchToDatabase(local.id);
    }
    await signOut();
    cloudStore.resetCloudState();
  };

  // ---------- note editor ----------

  // Sets which note the currently-focused pane shows - the actual title/
  // content/mode-flag loading happens reactively inside NotePane once its
  // noteId prop changes (see NotePane.svelte), mirroring how the editor
  // components one level down already resync on their own noteId prop.
  // This just owns the app-wide "what's selected" state and, when asked,
  // chases keyboard focus into the pane once it's rendered the new note.
  //
  // activeParentId (which parent new notes land under) deliberately stays
  // tied to the primary pane only, even when the secondary pane is
  // focused - keeping it pane-aware too wasn't worth the extra state for
  // what's a fairly rare combination (creating a note while the secondary
  // pane specifically has focus).
  const selectNote = (note: NoteRecord, focusEditor = true) => {
    if (focusedPane === 'secondary') {
      splitNoteId = note.id;
    } else {
      selectedId.set(note.id);
      activeParentId.set(note.parentId);
    }
    if (focusEditor) requestAnimationFrame(() => activePaneRef()?.focus());
  };

  // Shared onNavigate for both NotePane instances (wiki-link clicks,
  // backlink-popover clicks) - relies on a pane's own onFocus (fired on
  // mousedown, before the click that leads here) having already run, so
  // focusedPane correctly reflects whichever pane was actually clicked in
  // by the time selectNote below decides where to route the navigation.
  const handleNavigate = (id: number) => {
    const note = $notes.find((n) => n.id === id);
    if (note) selectNote(note, true);
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
  const openSearchResult = async (id: number, databaseId?: number | string) => {
    const needsSwitch =
      databaseId != null &&
      (typeof databaseId === 'string' ? databaseId !== $activeCloudNotebookId : $activeCloudNotebookId != null || databaseId !== $activeDatabaseId);
    if (needsSwitch) {
      // switchToDatabase/switchToCloudNotebook -> applyAppState (or its own
      // equivalent) already refreshes otherDatabaseNotes (when the toggle
      // is on) as part of its normal post-switch sequence - no need to do
      // it again here.
      const savedQuery = query;
      if (typeof databaseId === 'string') {
        await switchToCloudNotebook(databaseId);
      } else if (databaseId != null) {
        await switchToDatabase(databaseId);
      }
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
      activePaneRef()?.focus();
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

  // Saving, title-derivation, Markdown/Editor-mode toggling, Format, and
  // insert-at-cursor are all NotePane's own concerns now (see
  // NotePane.svelte) - these just dispatch to whichever pane is active.
  const insertAtCursor = (text: string) => activePaneRef()?.insertAtCursor(text);

  const formatActiveNote = () => void activePaneRef()?.format();

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
    const defaultTitle = dateTimeNoteNamesEnabled ? formatLocalTimestamp(new Date()) : 'Untitled';
    selectNote(await notesStore.createNoteIn(parentId, defaultTitle));
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
    if (!result) return;
    primaryPaneRef?.syncRenamedTitle(result.id, result.updated.title);
    secondaryPaneRef?.syncRenamedTitle(result.id, result.updated.title);
  };

  const duplicateNote = async (id: number) => {
    selectNote(await notesStore.duplicateNote(id));
  };

  // Exports a note's raw text content (markdown source, plain text, or
  // code - whatever's actually stored, no rendering/conversion) to a
  // user-chosen .txt file. Reads straight from the note record rather than
  // the currently-open noteText/title locals, since the right-clicked note
  // (sidebar) isn't necessarily the one currently open in the editor pane.
  const exportNoteToTxt = async (id: number) => {
    const note = $notes.find((n) => n.id === id);
    if (!note) return;
    // Filesystem-illegal characters on Windows (and awkward on macOS/Linux
    // too) - note titles are free-form text with no such restriction.
    const safeTitle = (note.title.trim() || 'Untitled').replace(/[\\/:*?"<>|]/g, '_');
    const picked = await saveFileDialog({
      defaultPath: `${safeTitle}.txt`,
      filters: [{ name: 'Text file', extensions: ['txt'] }],
    });
    if (!picked) return;
    try {
      await backupService.exportNoteText(picked, note.content);
      status.set('Exported');
    } catch (err) {
      status.set(err instanceof Error ? err.message : 'Export failed');
    }
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
    // notesStore.deleteNote already clears the selectedId store itself when
    // there's no next note - NotePane's own noteId-reactive load picks that
    // up and resets to its empty state, nothing to do here for that case.
    if (result.removedSelected && result.nextNote) {
      selectNote(result.nextNote);
    }
    // notesStore.deleteNote only knows about the selectedId store, not the
    // secondary pane's splitNoteId - fall it back the same way (to
    // whatever the primary pane fell back to, or empty) if the deleted
    // note or one of its now-deleted descendants was showing there.
    if (splitNoteId != null && (splitNoteId === id || descendantIds.has(splitNoteId))) {
      splitNoteId = result.nextNote?.id ?? null;
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
        { label: 'Copy', action: () => notesStore.copyNote(noteId) },
        { label: 'Cut', action: () => notesStore.cutNote(noteId) },
        { label: 'Export to .txt…', action: () => void exportNoteToTxt(noteId) },
        { label: '', separator: true },
        { label: locked ? 'Unlock' : 'Lock', action: () => void notesStore.toggleLock(noteId) },
        { label: '', separator: true },
        { label: 'Delete', danger: true, action: () => void deleteNoteById(noteId) },
      ],
    };
  };

  $: treeNodeProps = {
    expandedNotes: $expandedNotes,
    selectedNoteId: focusedNoteId,
    focusedKey: $focusedKey,
    renamingKey: $renamingKey,
    cutId: $clipboard?.mode === 'cut' ? $clipboard.id : null,
    draggingId: $draggingId,
    dropDisabledIds: $dropDisabledIds,
    onToggleExpand: notesStore.toggleExpand,
    onSelectNote: (id: number, databaseId?: number | string) => void openSearchResult(id, databaseId),
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
    // This handler also runs for the search input (see onSearchKeydown on
    // Footer) - j/k must not hijack normal typing there, so they're only
    // treated as vim-style up/down when the tree itself has focus.
    const inSearchInput = (event.target as HTMLElement | null)?.tagName === 'INPUT';
    const vimDown = vimModeEnabled && !inSearchInput && event.key === 'j';
    const vimUp = vimModeEnabled && !inSearchInput && event.key === 'k';

    // Alt+arrow is claimed globally (see handleKeydown) for reordering/
    // indenting notes - without this guard, a tree row with real keyboard
    // focus would ALSO move the tree's focus cursor or toggle expand/
    // collapse on top of that, since none of the branches below otherwise
    // check for Alt.
    if (event.altKey) return;

    if (event.key === 'ArrowDown' || vimDown) {
      event.preventDefault();
      const next = visibleFlat[Math.min(currentIndex + 1, visibleFlat.length - 1)];
      focusedKey.set(next?.key ?? visibleFlat[0].key);
    } else if (event.key === 'ArrowUp' || vimUp) {
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

  const setVimMode = (enabled: boolean) => {
    vimModeEnabled = enabled;
    void settingsService.saveVimMode(enabled);
  };

  const setDateTimeNoteNames = (enabled: boolean) => {
    dateTimeNoteNamesEnabled = enabled;
    void settingsService.saveDateTimeNoteNames(enabled);
  };

  // Re-reads every enabled plugin from disk and re-activates it - clears
  // and rebuilds all plugin contributions (see loadEnabledPlugins), so
  // this is also how a code change to a plugin's own files takes effect
  // without restarting FlashPad.
  const reloadPlugins = async () => {
    await loadEnabledPlugins(enabledPluginIds);
  };

  const setPluginEnabled = async (pluginId: string, enabled: boolean) => {
    enabledPluginIds = enabled ? [...enabledPluginIds, pluginId] : enabledPluginIds.filter((id) => id !== pluginId);
    await settingsService.saveEnabledPlugins(enabledPluginIds);
    await reloadPlugins();
  };

  // Per-note, toggled via Alt+R - Editor mode only (same treatment as
  // Format/Alt+F), since the line-number gutter is a CodeMirror-view
  // concern. Not gated on isLockedActive, since this is a display
  // preference rather than an edit to the note's protected text (and
  // update_note's lock guard only rejects title/content changes anyway, so
  // this always goes through even on a locked note). NotePane derives
  // showLineNumbers straight from the notes store (see NotePane.svelte),
  // so there's no local flag here to resync after the toggle.
  const toggleLineNumbers = async (id: number) => {
    if (!$notes.find((n) => n.id === id)?.isEditorMode) return;
    await notesStore.toggleLineNumbers(id);
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
            if ($selectedId != null) void notesStore.toggleLock($selectedId);
          } },
        { label: 'Export to .txt…', disabled: $selectedId == null, action: () => {
            if ($selectedId != null) void exportNoteToTxt($selectedId);
          } },
        { label: '', separator: true },
        { label: 'Delete', danger: true, action: () => {
            if ($selectedId != null) void deleteNoteById($selectedId);
          } },
      ],
    };
  };

  // Tracks physically-held keys ourselves (by event.code) rather than
  // trusting KeyboardEvent.repeat - WebKitGTK (the webview Tauri uses on
  // Linux) doesn't reliably set it, so relying on it alone still let a
  // held Alt+N (etc.) fire the shortcut multiple times. Cleared on keyup,
  // and defensively on blur too, in case a keyup is ever missed (e.g. the
  // window loses focus mid-press) - otherwise that key would look "stuck
  // held" and its shortcut would never fire again until reload.
  const heldKeyCodes = new Set<string>();

  const handleKeyup = (event: KeyboardEvent) => {
    heldKeyCodes.delete(event.code);
  };

  const handleWindowBlur = () => {
    heldKeyCodes.clear();
  };

  const handleKeydown = (event: KeyboardEvent) => {
    // Reordering (Alt+ArrowUp/Down): deliberately allowed to repeat while
    // held, like holding a plain arrow key to move through a list, so
    // checked before the single-fire guard below rather than through it.
    // Keyboard equivalent of dragging a row - works identically on every
    // platform, unlike HTML5 drag-and-drop (see tauri.windows.conf.json).
    if (event.altKey && (event.key === 'ArrowUp' || event.key === 'ArrowDown')) {
      event.preventDefault();
      if ($selectedId != null) void notesStore.moveNoteOrder($selectedId, event.key === 'ArrowUp' ? -1 : 1);
      return;
    }

    // Nesting (Alt+ArrowLeft/ArrowRight): same repeat-while-held treatment
    // as reordering above - right/"indent" nests under the previous
    // sibling, left/"outdent" promotes to the parent's level.
    if (event.altKey && (event.key === 'ArrowLeft' || event.key === 'ArrowRight')) {
      event.preventDefault();
      if ($selectedId != null) {
        void (event.key === 'ArrowRight' ? notesStore.indentNote($selectedId) : notesStore.outdentNote($selectedId));
      }
      return;
    }

    // Every shortcut here is a single, discrete action (create a note,
    // delete a note, insert a timestamp, ...) - none of them should repeat
    // just because a key was held a moment too long.
    if (heldKeyCodes.has(event.code)) return;
    heldKeyCodes.add(event.code);

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
      if (focusedNoteId != null) void notesStore.toggleLock(focusedNoteId);
    }

    if (event.altKey && event.key.toLowerCase() === 'd') {
      event.preventDefault();
      if (focusedNoteId != null) void deleteNoteById(focusedNoteId);
    }

    if (event.altKey && event.key.toLowerCase() === 'm') {
      event.preventDefault();
      activePaneRef()?.toggleMarkdown();
    }

    if (event.altKey && event.key.toLowerCase() === 'e') {
      event.preventDefault();
      if (focusedNoteId != null) void activePaneRef()?.toggleEditorMode();
    }

    if (event.altKey && event.key.toLowerCase() === 'b') {
      event.preventDefault();
      void cycleDatabase();
    }

    if (event.altKey && event.key.toLowerCase() === 't') {
      event.preventDefault();
      toggleMenuFocus();
    }

    if (event.altKey && event.key.toLowerCase() === 'o') {
      event.preventDefault();
      activePaneRef()?.openLinkAtCursor();
    }

    if (event.altKey && event.key.toLowerCase() === 'f') {
      event.preventDefault();
      void formatActiveNote();
    }

    if (event.altKey && event.key.toLowerCase() === 'r') {
      event.preventDefault();
      if (focusedNoteId != null) void toggleLineNumbers(focusedNoteId);
    }

    if (event.altKey && event.key.toLowerCase() === 'v') {
      event.preventDefault();
      toggleSplitView();
    }

    if (event.key === 'Escape') {
      if (contextMenu || shortcutsOpen || settingsOpen || markdownHelpOpen || confirmState || updateDetailsOpen || $activePluginForm || $activePluginMessage) return;
      event.preventDefault();
      void invoke('hide_window').catch(() => {
        status.set('Window hidden');
      });
    }
  };

  // Both editors write their own text/plain flavor from a copy/cut handler
  // (CodeMirror's copiedRange, ProseMirror's serializeForClipboard) and both
  // join lines with a bare LF. Blink's LF-to-CRLF conversion on Windows only
  // covers text IT puts on the clipboard, not text an editor set on the
  // event's DataTransfer, so multi-line text copied out of FlashPad lands in
  // Win32 edit controls (Notepad, FlashNote, most Oracle Forms fields) as a
  // single run-together line.
  //
  // Handled here on window rather than inside each editor: window is the
  // last stop in the bubble phase, so whatever the focused editor put on the
  // DataTransfer is already there to be rewritten, and the fix covers any
  // future copy source for free. The DataTransfer is still writable this
  // late in the dispatch. An empty text/plain means nothing intercepted the
  // event and the browser will populate the clipboard itself afterwards -
  // that path already converts, so leave it alone rather than hijacking it.
  const normalizeCopiedNewlines = (event: ClipboardEvent) => {
    if (!prefersCrlfClipboard()) return;
    const data = event.clipboardData;
    const text = data?.getData('text/plain');
    if (!data || !text) return;
    const crlf = toCrlfNewlines(text);
    if (crlf !== text) data.setData('text/plain', crlf);
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
      vimModeEnabled = settings.vimMode;
      dateTimeNoteNamesEnabled = settings.dateTimeNoteNames;
      document.documentElement.dataset.theme = theme;
      applyActivePalette();
      enabledPluginIds = settings.enabledPlugins;
    } catch (err) {
      console.error('FlashPad failed to load settings', err);
    }
    try {
      await ensurePluginsDirExists();
      await loadEnabledPlugins(enabledPluginIds);
    } catch (err) {
      console.error('FlashPad failed to load plugins', err);
    }
    try {
      await databaseService.init();
      hotkeySetting = await hotkeyService.get();

      // Resolve the Supabase session before deciding anything below - a
      // persisted cloud-notebook selection can only be resumed once it's
      // known whether there's actually a session to resume it with.
      await authReadyPromise;
      const persistedSource = loadActiveSource();
      // Best-effort, regardless of which source ends up active: populates
      // the notebooks list for the Cloud settings tab and for
      // hasSearchableOtherSources, without requiring Settings to be opened
      // first.
      if ($session) void cloudStore.refreshNotebooks().catch(() => {});

      // Always populate the local database list/id, regardless of which
      // source ends up active below - Alt+B cycling, the sidebar's active-
      // database indicator, and Settings all read `databases` directly, and
      // previously this only ran in the "local" branch, leaving that list
      // empty/stale for the entire session whenever the app resumed
      // straight into a cloud notebook (only backfilled once Settings
      // happened to be opened and closed).
      //
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

      if (persistedSource?.kind === 'cloud' && $session) {
        notesStore.setActiveNotesBackend(new CloudNotesService(persistedSource.notebookId));
        activeCloudNotebookId.set(persistedSource.notebookId);
        try {
          await initializeNotes();
        } catch (err) {
          await fallBackToLocal(err instanceof Error ? err.message : 'That notebook is no longer available - showing local notes.');
        }
      } else {
        if (persistedSource?.kind === 'cloud') {
          // No session - can't silently resume a cloud notebook with no
          // stored password.
          clearActiveSource();
        }

        if (appState && !appState.ready) {
          startupError.set(appState.error ?? 'The configured database is unavailable.');
        } else {
          await initializeNotes();
        }
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
    window.addEventListener('keyup', handleKeyup);
    window.addEventListener('blur', handleWindowBlur);
    window.addEventListener('copy', normalizeCopiedNewlines);
    window.addEventListener('cut', normalizeCopiedNewlines);
    return () => {
      window.removeEventListener('keydown', handleKeydown);
      window.removeEventListener('keyup', handleKeyup);
      window.removeEventListener('blur', handleWindowBlur);
      window.removeEventListener('copy', normalizeCopiedNewlines);
      window.removeEventListener('cut', normalizeCopiedNewlines);
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
    onFormat={formatActiveNote}
    editorModeEnabled={isEditorModeActive}
    onSearch={() => activePaneRef()?.openSearch()}
    {splitViewEnabled}
    onToggleSplitView={toggleSplitView}
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

  <div class="content-column">
    <div class="panes">
      <div class="pane-slot" style="flex-grow: {splitViewEnabled ? splitRatio : 1}">
        <NotePane
          bind:this={primaryPaneRef}
          bind:isMarkdownActive={primaryIsMarkdownActive}
          bind:isLockedActive={primaryIsLockedActive}
          bind:isEditorModeActive={primaryIsEditorModeActive}
          noteId={$selectedId}
          onNavigate={handleNavigate}
          onOpenLink={openLink}
          vimMode={vimModeEnabled}
          {theme}
          focused={!splitViewEnabled || focusedPane === 'primary'}
          onFocus={() => focusPane('primary')}
        />
      </div>

      {#if splitViewEnabled}
        <PaneResizer bind:ratio={splitRatio} />

        <div class="pane-slot" style="flex-grow: {1 - splitRatio}">
          <NotePane
            bind:this={secondaryPaneRef}
            bind:isMarkdownActive={secondaryIsMarkdownActive}
            bind:isLockedActive={secondaryIsLockedActive}
            bind:isEditorModeActive={secondaryIsEditorModeActive}
            noteId={splitNoteId}
            onNavigate={handleNavigate}
            onOpenLink={openLink}
            vimMode={vimModeEnabled}
            {theme}
            focused={focusedPane === 'secondary'}
            onFocus={() => focusPane('secondary')}
          />
        </div>
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
      onToggleMarkdown={() => activePaneRef()?.toggleMarkdown()}
    />
  </div>
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
    vimMode={vimModeEnabled}
    onVimModeChange={setVimMode}
    dateTimeNoteNames={dateTimeNoteNamesEnabled}
    onDateTimeNoteNamesChange={setDateTimeNoteNames}
    {enabledPluginIds}
    onSetPluginEnabled={setPluginEnabled}
    onReloadPlugins={reloadPlugins}
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
    onSwitchCloudNotebook={switchToCloudNotebook}
    onSignOut={handleSignOut}
    activeCloudNotebookId={$activeCloudNotebookId}
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

{#if $activePluginForm}
  <PluginFormDialog
    title={$activePluginForm.title}
    fields={$activePluginForm.fields}
    submitLabel={$activePluginForm.submitLabel}
    onSubmit={$activePluginForm.onSubmit}
    onClose={() => activePluginForm.set(null)}
  />
{/if}

{#if $activePluginMessage}
  <PluginMessageDialog
    title={$activePluginMessage.title}
    message={$activePluginMessage.message}
    linkLabel={$activePluginMessage.linkLabel}
    linkUrl={$activePluginMessage.linkUrl}
    onClose={() => activePluginMessage.set(null)}
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

  :global(body.resizing-sidebar),
  :global(body.resizing-panes) {
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

  .content-column {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }

  .panes {
    flex: 1;
    display: flex;
    min-height: 0;
  }

  .pane-slot {
    display: flex;
    min-width: 0;
    min-height: 0;
    flex-shrink: 1;
    flex-basis: 0;
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

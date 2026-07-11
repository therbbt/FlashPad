<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { NotesService, type NoteRecord } from './lib/services/notesService';
  import { SettingsService, type FlashPadSettings } from './lib/services/settingsService';
  import { HotkeyService } from './lib/services/hotkeyService';
  import { DatabaseService } from './lib/services/databaseService';
  import TreeNode, { type TreeItem } from './lib/components/TreeNode.svelte';
  import ContextMenu, { type ContextMenuItem } from './lib/components/ContextMenu.svelte';
  import ShortcutsPanel from './lib/components/ShortcutsPanel.svelte';
  import SettingsPanel from './lib/components/SettingsPanel.svelte';

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
  let shortcutsOpen = false;
  let settingsOpen = false;
  let hotkeySetting = 'Alt+S';
  let sidebarWidth = DEFAULT_SIDEBAR_WIDTH;
  let isResizingSidebar = false;

  let noteText = '';
  let title = 'Untitled';
  let titleAutoDerive = true;
  let query = '';
  let status = 'Ready';
  let theme: FlashPadSettings['theme'] = 'dark';
  let textarea: HTMLTextAreaElement;
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
    noteList.forEach((n) => nodeById.set(n.id, { id: n.id, title: n.title, children: [] }));

    const roots: TreeItem[] = [];
    noteList.forEach((n) => {
      const node = nodeById.get(n.id)!;
      const parent = n.parentId != null ? nodeById.get(n.parentId) : undefined;
      if (parent) parent.children.push(node);
      else roots.push(node);
    });

    const sortItems = (items: TreeItem[]) => {
      items.sort((a, b) => a.title.localeCompare(b.title, undefined, { sensitivity: 'base' }));
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
    ? searchResults.map((n) => ({ key: `note:${n.id}`, item: { id: n.id, title: n.title, children: [] } as TreeItem }))
    : flattenVisible(tree, expandedNotes);
  $: if (visibleFlat.length && !visibleFlat.some((v) => v.key === focusedKey)) {
    focusedKey = visibleFlat[0].key;
  }
  $: currentPath = activeParentId != null ? (notes.find((n) => n.id === activeParentId) ? notePath(notes.find((n) => n.id === activeParentId)!) : 'Notes') : 'Notes';
  $: searchMatchIndex = isSearching ? searchResults.findIndex((n) => n.id === selectedId) : -1;

  // ---------- data loading ----------

  const refreshNotes = async () => {
    notes = await notesService.list();
  };

  const refreshAll = async () => {
    await refreshNotes();
    status = 'Refreshed';
  };

  // ---------- note editor ----------

  const deriveTitleFromContent = (content: string): string => {
    const firstLine = content.split('\n').find((line) => line.trim().length > 0)?.trim() ?? '';
    if (!firstLine) return 'Untitled';
    return firstLine.length > 80 ? firstLine.slice(0, 80) : firstLine;
  };

  const selectNote = (note: NoteRecord, focusEditor = true) => {
    selectedId = note.id;
    activeParentId = note.parentId;
    title = note.title;
    noteText = note.content;
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
    if (titleAutoDerive) {
      title = deriveTitleFromContent(noteText);
    }
    scheduleSave();
  };

  const handleTitleInput = () => {
    titleAutoDerive = false;
    scheduleSave();
  };

  const insertAtCursor = (text: string) => {
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
      'Welcome to FlashPad',
      '',
      `Press ${hotkeySetting} anywhere to open FlashPad instantly.`,
      'Press Esc to hide it - it keeps running in the tray.',
      '',
      'Right-click a note (or the sidebar background) to create a note,',
      'rename, duplicate, move, or delete. Any note can hold subnotes -',
      'once it has one, it shows a folder icon: click it to open its own',
      'content, click the little arrow to expand or collapse its subnotes.',
      '',
      'Use the search box at the bottom to find notes, with prev/next',
      'buttons (or Enter / Shift+Enter) to step through matches.',
      '',
      'Alt+1  Insert a divider',
      'Alt+2  Insert a timestamp',
      'Alt+3  Insert a dateline',
      '',
      'The gear icon in Settings lets you launch FlashPad at login and',
      'change the hotkey above to whatever you like.',
      '',
      'Start typing to replace this note.',
    ].join('\n');

    const created = await notesService.create({ title: 'Welcome to FlashPad', content, parentId: null });
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

  const moveNoteTo = async (id: number, parentId: number | null) => {
    try {
      const updated = await notesService.move(id, parentId);
      notes = notes.map((n) => (n.id === id ? updated : n));
      if (selectedId === id) activeParentId = parentId;
      status = 'Moved';
    } catch (err) {
      status = err instanceof Error ? err.message : 'Move failed';
    }
  };

  const duplicateNote = async (id: number) => {
    const created = await notesService.duplicate(id);
    notes = [created, ...notes];
    selectNote(created);
    status = 'Duplicated';
  };

  const deleteNoteById = async (id: number) => {
    const descendantIds = collectDescendantNoteIds(id);
    const removedIds = new Set([id, ...descendantIds]);
    const message =
      descendantIds.size > 0
        ? `Delete this note and ${descendantIds.size} note${descendantIds.size === 1 ? '' : 's'} inside it? This cannot be undone.`
        : 'Delete this note? This cannot be undone.';
    if (!confirm(message)) return;

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

  const openBackgroundMenu = (event: MouseEvent) => {
    contextMenu = {
      x: event.clientX,
      y: event.clientY,
      items: [
        { label: 'New note', action: () => void createNoteIn(activeParentId) },
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
    contextMenu = {
      x: event.clientX,
      y: event.clientY,
      items: [
        { label: 'Open', action: () => void openNote(noteId, true) },
        { label: 'New subnote', action: () => void createNoteIn(noteId) },
        { label: 'Rename', action: () => (renamingKey = `note:${noteId}`) },
        { label: 'Duplicate', action: () => void duplicateNote(noteId) },
        { label: 'Move to…', submenu: buildMoveTargetItems((target) => void moveNoteTo(noteId, target), noteId) },
        { label: '', separator: true },
        { label: 'Delete', danger: true, action: () => void deleteNoteById(noteId) },
      ],
    };
  };

  $: treeNodeProps = {
    expandedNotes,
    selectedNoteId: selectedId,
    focusedKey,
    renamingKey,
    onToggleExpand: toggleExpand,
    onSelectNote: (id: number) => void openNote(id),
    onNoteContextMenu: openNoteMenu,
    onFocusItem: (key: string) => {
      focusedKey = key;
      treeEl?.focus();
    },
    onRenameCommit: commitRename,
    onRenameCancel: () => (renamingKey = null),
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

  const toggleTheme = () => {
    theme = theme === 'dark' ? 'light' : 'dark';
    document.documentElement.dataset.theme = theme;
    void settingsService.saveTheme(theme);
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

    if (event.key === 'Escape') {
      if (contextMenu || shortcutsOpen || settingsOpen) return;
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
      await databaseService.init();
      const settings = await settingsService.load();
      theme = settings.theme;
      document.documentElement.dataset.theme = theme;
      hotkeySetting = await hotkeyService.get();
      await refreshAll();
      if (notes.length) {
        selectNote(notes[0]);
      } else {
        await createWelcomeNote();
      }
      requestAnimationFrame(() => textarea?.focus());
    } catch (err) {
      console.error('FlashPad failed to initialize', err);
      status = err instanceof Error ? err.message : 'Startup error';
    }
    window.addEventListener('keydown', handleKeydown);
    return () => window.removeEventListener('keydown', handleKeydown);
  });
</script>

<svelte:head>
  <title>FlashPad</title>
</svelte:head>

<div class="app-shell">
  <div class="action-toolbar">
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

    <button class="toolbar-btn shortcuts-btn" on:click={() => (shortcutsOpen = true)} aria-label="Keyboard shortcuts">
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

  <div class="shell">
  <aside class="sidebar" style="width: {sidebarWidth}px">
    <div class="sidebar-top">
      <h1>FlashPad</h1>
    </div>

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
          <TreeNode item={{ id: note.id, title: note.title, children: [] }} depth={0} {...treeNodeProps} />
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
    <header class="topbar">
      <div class="title-block">
        <input bind:value={title} class="title" placeholder="Untitled" on:input={handleTitleInput} />
        <span class="breadcrumb">{currentPath}</span>
      </div>
    </header>

    <textarea
      bind:this={textarea}
      bind:value={noteText}
      class="editor"
      placeholder="Start typing instantly..."
      on:input={handleEditorInput}
    ></textarea>

    <footer class="footer">
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
      <span class="status">{status}</span>
    </footer>
  </section>
  </div>
</div>

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
    onClose={() => (settingsOpen = false)}
  />
{/if}

<style>
  :global(html[data-theme='light']) {
    color-scheme: light;
    --bg: #f4f6fb;
    --panel: #ffffff;
    --panel-2: #edf2f7;
    --text: #111827;
    --muted: #4b5563;
    --border: rgba(17, 24, 39, 0.1);
    --accent: #2563eb;
    --accent-soft: rgba(37, 99, 235, 0.14);
  }

  :global(html:not([data-theme='light'])) {
    color-scheme: dark;
    --bg: #16161a;
    --panel: #1e1e22;
    --panel-2: #28282d;
    --text: #ecebe7;
    --muted: #97958d;
    --border: rgba(255,255,255,0.08);
    --accent: #e0a458;
    --accent-soft: rgba(224, 164, 88, 0.18);
  }

  :global(body.resizing-sidebar) {
    cursor: col-resize;
    user-select: none;
  }

  .app-shell {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--bg);
    color: var(--text);
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

  .sidebar-top {
    display: flex;
    align-items: center;
  }

  .sidebar h1 {
    margin: 0;
    font-size: 1rem;
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

  .toolbar-btn.shortcuts-btn {
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

  .footer span:last-child {
    margin-left: auto;
  }
</style>

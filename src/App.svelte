<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { NotesService, type NoteRecord } from './lib/services/notesService';
  import { FoldersService, type FolderRecord } from './lib/services/foldersService';
  import { SettingsService, type FlashPadSettings } from './lib/services/settingsService';
  import { HotkeyService } from './lib/services/hotkeyService';
  import { DatabaseService } from './lib/services/databaseService';
  import TreeNode, { type TreeItem } from './lib/components/TreeNode.svelte';
  import ContextMenu, { type ContextMenuItem } from './lib/components/ContextMenu.svelte';
  import ShortcutsPanel from './lib/components/ShortcutsPanel.svelte';

  const notesService = new NotesService();
  const foldersService = new FoldersService();
  const settingsService = new SettingsService();
  const hotkeyService = new HotkeyService();
  const databaseService = new DatabaseService();

  const EXPANDED_KEY = 'flashpad.expandedFolders';
  const SIDEBAR_WIDTH_KEY = 'flashpad.sidebarWidth';
  const SIDEBAR_MIN_WIDTH = 80;
  const SIDEBAR_MAX_WIDTH = 480;
  const DEFAULT_SIDEBAR_WIDTH = 260;

  let notes: NoteRecord[] = [];
  let folders: FolderRecord[] = [];
  let selectedId: number | null = null;
  let selectedFolderId: number | null = null;
  let expandedFolders: Set<number> = new Set();
  let focusedKey: string | null = null;
  let renamingKey: string | null = null;
  let contextMenu: { x: number; y: number; items: ContextMenuItem[] } | null = null;
  let shortcutsOpen = false;
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
  let noteActionsButton: HTMLButtonElement;
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
      window.localStorage.setItem(EXPANDED_KEY, JSON.stringify([...expandedFolders]));
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

  const buildTree = (folderList: FolderRecord[], noteList: NoteRecord[]): TreeItem[] => {
    const nodeById = new Map<number, TreeItem & { type: 'folder' }>();
    folderList.forEach((f) => nodeById.set(f.id, { type: 'folder', id: f.id, name: f.name, children: [] }));

    const roots: TreeItem[] = [];
    folderList.forEach((f) => {
      const node = nodeById.get(f.id)!;
      const parent = f.parentId != null ? nodeById.get(f.parentId) : undefined;
      if (parent) parent.children.push(node);
      else roots.push(node);
    });

    noteList.forEach((n) => {
      const noteNode: TreeItem = { type: 'note', id: n.id, title: n.title };
      const parent = n.folderId != null ? nodeById.get(n.folderId) : undefined;
      if (parent) parent.children.push(noteNode);
      else roots.push(noteNode);
    });

    const sortItems = (items: TreeItem[]) => {
      items.sort((a, b) => {
        if (a.type !== b.type) return a.type === 'folder' ? -1 : 1;
        const an = a.type === 'folder' ? a.name : a.title;
        const bn = b.type === 'folder' ? b.name : b.title;
        return an.localeCompare(bn, undefined, { sensitivity: 'base' });
      });
      items.forEach((item) => item.type === 'folder' && sortItems(item.children));
    };
    sortItems(roots);
    return roots;
  };

  const flattenVisible = (items: TreeItem[], expanded: Set<number>): { key: string; item: TreeItem }[] => {
    const out: { key: string; item: TreeItem }[] = [];
    const walk = (list: TreeItem[]) => {
      for (const item of list) {
        const key = item.type === 'folder' ? `folder:${item.id}` : `note:${item.id}`;
        out.push({ key, item });
        if (item.type === 'folder' && expanded.has(item.id)) walk(item.children);
      }
    };
    walk(items);
    return out;
  };

  const folderPath = (folder: FolderRecord): string => {
    const parts: string[] = [folder.name];
    let current: FolderRecord | undefined = folder;
    while (current && current.parentId != null) {
      current = folders.find((f) => f.id === current!.parentId);
      if (current) parts.unshift(current.name);
    }
    return parts.join(' / ');
  };

  const collectDescendantIds = (rootId: number): Set<number> => {
    const ids = new Set<number>();
    const queue = [rootId];
    while (queue.length) {
      const current = queue.pop()!;
      for (const f of folders) {
        if (f.parentId === current && !ids.has(f.id)) {
          ids.add(f.id);
          queue.push(f.id);
        }
      }
    }
    return ids;
  };

  $: tree = buildTree(folders, notes);
  $: normalizedQuery = query.trim().toLowerCase();
  $: isSearching = normalizedQuery.length > 0;
  $: searchResults = isSearching
    ? notes
        .filter((n) => `${n.title} ${n.content}`.toLowerCase().includes(normalizedQuery))
        .sort((a, b) => a.title.localeCompare(b.title, undefined, { sensitivity: 'base' }))
    : [];
  $: visibleFlat = isSearching
    ? searchResults.map((n) => ({ key: `note:${n.id}`, item: { type: 'note', id: n.id, title: n.title } as TreeItem }))
    : flattenVisible(tree, expandedFolders);
  $: if (visibleFlat.length && !visibleFlat.some((v) => v.key === focusedKey)) {
    focusedKey = visibleFlat[0].key;
  }
  $: currentFolderName = selectedFolderId != null ? (folders.find((f) => f.id === selectedFolderId) ? folderPath(folders.find((f) => f.id === selectedFolderId)!) : 'Notes') : 'Notes';

  // ---------- data loading ----------

  const refreshNotes = async () => {
    notes = await notesService.list();
  };

  const refreshFolders = async () => {
    folders = await foldersService.list();
  };

  const refreshAll = async () => {
    await Promise.all([refreshNotes(), refreshFolders()]);
    status = 'Refreshed';
  };

  // ---------- note editor ----------

  const deriveTitleFromContent = (content: string): string => {
    const firstLine = content.split('\n').find((line) => line.trim().length > 0)?.trim() ?? '';
    if (!firstLine) return 'Untitled';
    return firstLine.length > 80 ? firstLine.slice(0, 80) : firstLine;
  };

  const selectNote = (note: NoteRecord) => {
    selectedId = note.id;
    selectedFolderId = note.folderId;
    title = note.title;
    noteText = note.content;
    titleAutoDerive = note.title === 'Untitled' || note.title.trim() === '';
    requestAnimationFrame(() => textarea?.focus());
  };

  const openNote = async (id: number) => {
    const note = notes.find((n) => n.id === id);
    if (note) selectNote(note);
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

  const insertTimestamp = () => {
    const settings = settingsService.getCached();
    const stamp = settingsService.renderTimestamp(settings.timestampFormat);
    const start = textarea.selectionStart;
    const end = textarea.selectionEnd;
    noteText = `${noteText.slice(0, start)}${stamp}${noteText.slice(end)}`;
    requestAnimationFrame(() => {
      const cursor = start + stamp.length;
      textarea.focus();
      textarea.setSelectionRange(cursor, cursor);
    });
    handleEditorInput();
  };

  // ---------- creation ----------

  const createNoteIn = async (folderId: number | null) => {
    const created = await notesService.create({ title: 'Untitled', content: '', folderId });
    notes = [created, ...notes];
    if (folderId != null && !expandedFolders.has(folderId)) {
      expandedFolders = new Set(expandedFolders).add(folderId);
      saveExpanded();
    }
    selectNote(created);
    focusedKey = `note:${created.id}`;
    status = 'New note';
  };

  const createFolderIn = async (parentId: number | null) => {
    const created = await foldersService.create('New folder', parentId);
    folders = [...folders, created];
    if (parentId != null && !expandedFolders.has(parentId)) {
      expandedFolders = new Set(expandedFolders).add(parentId);
      saveExpanded();
    }
    renamingKey = `folder:${created.id}`;
    focusedKey = `folder:${created.id}`;
    status = 'New folder';
  };

  // ---------- rename / move / delete / duplicate ----------

  const commitRename = async (key: string, value: string) => {
    renamingKey = null;
    const trimmed = value.trim();
    if (!trimmed) return;

    if (key.startsWith('folder:')) {
      const id = Number(key.slice('folder:'.length));
      const updated = await foldersService.rename(id, trimmed);
      folders = folders.map((f) => (f.id === id ? updated : f));
    } else {
      const id = Number(key.slice('note:'.length));
      const updated = await notesService.save({ id, title: trimmed });
      notes = notes.map((n) => (n.id === id ? updated : n));
      if (selectedId === id) {
        title = trimmed;
        titleAutoDerive = false;
      }
    }
  };

  const buildMoveTargetItems = (onPick: (folderId: number | null) => void, excludeFolderId?: number): ContextMenuItem[] => {
    const descendantIds = excludeFolderId != null ? collectDescendantIds(excludeFolderId) : new Set<number>();
    const eligible = folders
      .filter((f) => f.id !== excludeFolderId && !descendantIds.has(f.id))
      .sort((a, b) => folderPath(a).localeCompare(folderPath(b), undefined, { sensitivity: 'base' }));
    return [
      { label: 'Notes (root)', action: () => onPick(null) },
      ...eligible.map((f) => ({ label: folderPath(f), action: () => onPick(f.id) })),
    ];
  };

  const moveNoteTo = async (id: number, folderId: number | null) => {
    const updated = await notesService.move(id, folderId);
    notes = notes.map((n) => (n.id === id ? updated : n));
    if (selectedId === id) selectedFolderId = folderId;
    status = 'Moved';
  };

  const moveFolderTo = async (id: number, parentId: number | null) => {
    try {
      const updated = await foldersService.move(id, parentId);
      folders = folders.map((f) => (f.id === id ? updated : f));
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
    if (!confirm('Delete this note? This cannot be undone.')) return;
    await notesService.delete(id);
    notes = notes.filter((n) => n.id !== id);
    if (selectedId === id) {
      selectedId = null;
      if (notes.length) {
        selectNote(notes[0]);
      } else {
        title = 'Untitled';
        noteText = '';
        selectedFolderId = null;
      }
    }
    status = 'Deleted';
  };

  const deleteFolder = async (id: number) => {
    const descendantIds = collectDescendantIds(id);
    const removedFolderIds = new Set([id, ...descendantIds]);
    const affectedCount = notes.filter((n) => n.folderId != null && removedFolderIds.has(n.folderId)).length;
    const message =
      affectedCount > 0
        ? `Delete this folder and ${affectedCount} note${affectedCount === 1 ? '' : 's'} inside it? This cannot be undone.`
        : 'Delete this folder?';
    if (!confirm(message)) return;

    await foldersService.delete(id);
    folders = folders.filter((f) => !removedFolderIds.has(f.id));
    const deletedNoteIds = new Set(notes.filter((n) => n.folderId != null && removedFolderIds.has(n.folderId)).map((n) => n.id));
    notes = notes.filter((n) => !deletedNoteIds.has(n.id));
    if (selectedFolderId != null && removedFolderIds.has(selectedFolderId)) {
      selectedFolderId = null;
    }
    if (selectedId != null && deletedNoteIds.has(selectedId)) {
      selectedId = null;
      if (notes.length) {
        selectNote(notes[0]);
      } else {
        title = 'Untitled';
        noteText = '';
        selectedFolderId = null;
      }
    }
    status = 'Folder deleted';
  };

  // ---------- tree state ----------

  const toggleFolder = (id: number) => {
    const next = new Set(expandedFolders);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    expandedFolders = next;
    saveExpanded();
    selectedFolderId = id;
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
        { label: 'New folder', action: () => void createFolderIn(selectedFolderId) },
        { label: 'New note', action: () => void createNoteIn(selectedFolderId) },
        { label: '', separator: true },
        { label: 'Refresh', action: () => void refreshAll() },
        { label: 'Collapse all', action: () => {
            expandedFolders = new Set();
            saveExpanded();
          } },
      ],
    };
  };

  const openFolderMenu = (event: MouseEvent, folderId: number) => {
    contextMenu = {
      x: event.clientX,
      y: event.clientY,
      items: [
        { label: 'New note inside folder', action: () => void createNoteIn(folderId) },
        { label: 'New subfolder', action: () => void createFolderIn(folderId) },
        { label: 'Rename', action: () => (renamingKey = `folder:${folderId}`) },
        { label: 'Move', submenu: buildMoveTargetItems((target) => void moveFolderTo(folderId, target), folderId) },
        { label: '', separator: true },
        { label: 'Delete', danger: true, action: () => void deleteFolder(folderId) },
      ],
    };
  };

  const openNoteMenu = (event: MouseEvent, noteId: number) => {
    contextMenu = {
      x: event.clientX,
      y: event.clientY,
      items: [
        { label: 'Open', action: () => void openNote(noteId) },
        { label: 'Rename', action: () => (renamingKey = `note:${noteId}`) },
        { label: 'Duplicate', action: () => void duplicateNote(noteId) },
        { label: 'Move to folder', submenu: buildMoveTargetItems((target) => void moveNoteTo(noteId, target)) },
        { label: '', separator: true },
        { label: 'Delete', danger: true, action: () => void deleteNoteById(noteId) },
      ],
    };
  };

  $: treeNodeProps = {
    expandedFolders,
    selectedNoteId: selectedId,
    focusedKey,
    renamingKey,
    onToggleFolder: toggleFolder,
    onSelectNote: (id: number) => void openNote(id),
    onFolderContextMenu: openFolderMenu,
    onNoteContextMenu: openNoteMenu,
    onFocusItem: (key: string) => (focusedKey = key),
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
      if (entry?.item.type === 'folder' && !expandedFolders.has(entry.item.id)) {
        event.preventDefault();
        toggleFolder(entry.item.id);
      }
    } else if (event.key === 'ArrowLeft' && !isSearching) {
      const entry = visibleFlat[currentIndex];
      if (entry?.item.type === 'folder' && expandedFolders.has(entry.item.id)) {
        event.preventDefault();
        toggleFolder(entry.item.id);
      }
    } else if (event.key === 'Enter') {
      const entry = visibleFlat[currentIndex];
      if (entry) {
        event.preventDefault();
        if (entry.item.type === 'note') {
          void openNote(entry.item.id);
        } else {
          toggleFolder(entry.item.id);
        }
      }
    }
  };

  const toggleTheme = () => {
    theme = theme === 'dark' ? 'light' : 'dark';
    document.documentElement.dataset.theme = theme;
    void settingsService.saveTheme(theme);
  };

  const openNoteActionsMenu = () => {
    if (!noteActionsButton) return;
    const rect = noteActionsButton.getBoundingClientRect();
    contextMenu = {
      x: rect.right - 170,
      y: rect.bottom + 4,
      items: [
        { label: 'Insert timestamp', action: () => insertTimestamp() },
        { label: 'Delete note', danger: true, action: () => {
            if (selectedId != null) void deleteNoteById(selectedId);
          } },
      ],
    };
  };

  const handleKeydown = (event: KeyboardEvent) => {
    const isShortcut = (event.ctrlKey || event.metaKey) && event.shiftKey && event.key.toLowerCase() === 't';
    if (isShortcut) {
      event.preventDefault();
      insertTimestamp();
    }

    if (event.key === 'Escape') {
      if (contextMenu || shortcutsOpen) return;
      event.preventDefault();
      void invoke('hide_window').catch(() => {
        status = 'Window hidden';
      });
    }
  };

  onMount(async () => {
    expandedFolders = loadExpanded();
    sidebarWidth = loadSidebarWidth();
    try {
      await databaseService.init();
      const settings = await settingsService.load();
      theme = settings.theme;
      hotkeySetting = settings.hotkey;
      document.documentElement.dataset.theme = theme;
      try {
        await Promise.race([
          hotkeyService.register(settings.hotkey),
          new Promise((_, reject) => setTimeout(() => reject(new Error('hotkey register timed out')), 3000)),
        ]);
      } catch (err) {
        console.error('Failed to register global hotkey', err);
      }
      await refreshAll();
      if (notes.length) {
        selectNote(notes[0]);
      } else {
        await createNoteIn(null);
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
      on:contextmenu|preventDefault|self={openBackgroundMenu}
    >
      {#if isSearching}
        {#each searchResults as note (note.id)}
          <TreeNode item={{ type: 'note', id: note.id, title: note.title }} depth={0} {...treeNodeProps} />
        {/each}
        {#if !searchResults.length}
          <p class="empty-hint">No matches</p>
        {/if}
      {:else}
        {#each tree as item (item.type + ':' + item.id)}
          <TreeNode {item} depth={0} {...treeNodeProps} />
        {/each}
        {#if !tree.length}
          <p class="empty-hint">Right-click to create a folder or note</p>
        {/if}
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
      <button class="theme-toggle" on:click={() => (shortcutsOpen = true)} aria-label="Keyboard shortcuts">
        <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round">
          <circle cx="8" cy="8" r="6.5" />
          <path d="M6.1 6.2a1.9 1.9 0 1 1 2.7 1.7c-.7.35-.9.7-.9 1.4" stroke-linejoin="round" />
          <circle cx="8" cy="11.4" r="0.15" fill="currentColor" />
        </svg>
      </button>
    </div>
  </aside>

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="sidebar-resizer" class:active={isResizingSidebar} on:mousedown={startSidebarResize}></div>

  <section class="editor-pane">
    <header class="topbar">
      <div class="title-block">
        <input bind:value={title} class="title" placeholder="Untitled" on:input={handleTitleInput} />
        <span class="breadcrumb">{currentFolderName}</span>
      </div>
      <div class="toolbar">
        <button class="icon-btn" bind:this={noteActionsButton} on:click={openNoteActionsMenu} aria-label="Note actions">
          <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
            <circle cx="3" cy="8" r="1.4" />
            <circle cx="8" cy="8" r="1.4" />
            <circle cx="13" cy="8" r="1.4" />
          </svg>
        </button>
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
      <input class="search-input" bind:value={query} on:keydown={handleTreeKeydown} placeholder="Search notes" />
      <span>Ctrl+Shift+T · Esc hides</span>
    </footer>
  </section>
</div>

{#if contextMenu}
  <ContextMenu x={contextMenu.x} y={contextMenu.y} items={contextMenu.items} onClose={closeContextMenu} />
{/if}

{#if shortcutsOpen}
  <ShortcutsPanel hotkey={hotkeySetting} onClose={() => (shortcutsOpen = false)} />
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
  }

  :global(html:not([data-theme='light'])) {
    color-scheme: dark;
    --bg: #0f1115;
    --panel: #151923;
    --panel-2: #1b2130;
    --text: #f3f6fb;
    --muted: #98a2b3;
    --border: rgba(255,255,255,0.08);
    --accent: #60a5fa;
  }

  :global(body.resizing-sidebar) {
    cursor: col-resize;
    user-select: none;
  }

  .shell {
    display: flex;
    height: 100%;
    background: var(--bg);
    color: var(--text);
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

  .sidebar-resizer:hover,
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

  .toolbar {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 1.75rem;
    height: 1.75rem;
    border: 1px solid var(--border);
    border-radius: 0.5rem;
    background: var(--panel-2);
    color: var(--muted);
    padding: 0;
  }

  .icon-btn:hover {
    color: var(--text);
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

  .footer span:last-child {
    margin-left: auto;
  }
</style>

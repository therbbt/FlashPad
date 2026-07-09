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

  const notesService = new NotesService();
  const foldersService = new FoldersService();
  const settingsService = new SettingsService();
  const hotkeyService = new HotkeyService();
  const databaseService = new DatabaseService();

  const EXPANDED_KEY = 'flashpad.expandedFolders';

  let notes: NoteRecord[] = [];
  let folders: FolderRecord[] = [];
  let selectedId: number | null = null;
  let selectedFolderId: number | null = null;
  let expandedFolders: Set<number> = new Set();
  let focusedKey: string | null = null;
  let renamingKey: string | null = null;
  let contextMenu: { x: number; y: number; items: ContextMenuItem[] } | null = null;

  let noteText = '';
  let title = 'Untitled';
  let titleAutoDerive = true;
  let query = '';
  let status = 'Ready';
  let theme: FlashPadSettings['theme'] = 'dark';
  let textarea: HTMLTextAreaElement;
  let treeEl: HTMLDivElement;
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

  const handleKeydown = (event: KeyboardEvent) => {
    const isShortcut = (event.ctrlKey || event.metaKey) && event.shiftKey && event.key.toLowerCase() === 't';
    if (isShortcut) {
      event.preventDefault();
      insertTimestamp();
    }

    if (event.key === 'Escape') {
      event.preventDefault();
      void invoke('hide_window').catch(() => {
        status = 'Window hidden';
      });
    }
  };

  onMount(async () => {
    expandedFolders = loadExpanded();
    try {
      await databaseService.init();
      const settings = await settingsService.load();
      theme = settings.theme;
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
  <aside class="sidebar">
    <div class="sidebar-top">
      <div>
        <h1>FlashPad</h1>
        <p>Instant capture, local-first</p>
      </div>
      <button class="ghost" on:click={toggleTheme}>{theme === 'dark' ? '☀' : '☾'}</button>
    </div>

    <label class="field">
      <span>Search</span>
      <input bind:value={query} on:keydown={handleTreeKeydown} placeholder="Search notes" />
    </label>

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
  </aside>

  <section class="editor-pane">
    <header class="topbar">
      <div class="title-block">
        <input bind:value={title} class="title" placeholder="Untitled" on:input={handleTitleInput} />
        <span class="breadcrumb">{currentFolderName}</span>
      </div>
      <div class="toolbar">
        <button class="ghost" on:click={insertTimestamp}>Timestamp</button>
        <button class="danger" on:click={() => selectedId != null && deleteNoteById(selectedId)}>Delete</button>
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
      <span>{status}</span>
      <span>Ctrl+Shift+T · Esc hides</span>
    </footer>
  </section>
</div>

{#if contextMenu}
  <ContextMenu x={contextMenu.x} y={contextMenu.y} items={contextMenu.items} onClose={closeContextMenu} />
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

  .shell {
    display: flex;
    height: 100%;
    background: var(--bg);
    color: var(--text);
  }

  .sidebar {
    width: 260px;
    padding: 0.75rem;
    border-right: 1px solid var(--border);
    background: var(--panel);
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    min-height: 0;
  }

  .sidebar-top {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .sidebar h1 {
    margin: 0;
    font-size: 1rem;
  }

  .sidebar p {
    margin: 0.2rem 0 0;
    color: var(--muted);
    font-size: 0.8rem;
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

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    color: var(--muted);
    font-size: 0.75rem;
  }

  .field input {
    border: 1px solid var(--border);
    border-radius: 0.5rem;
    background: var(--panel-2);
    color: inherit;
    padding: 0.45rem 0.6rem;
  }

  .ghost,
  .danger {
    border: 1px solid var(--border);
    border-radius: 0.6rem;
    padding: 0.5rem 0.65rem;
    background: var(--panel-2);
    color: inherit;
  }

  .danger {
    background: rgba(220, 38, 38, 0.12);
    color: #ef4444;
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
    justify-content: space-between;
    padding: 0.6rem 0.9rem;
    border-top: 1px solid var(--border);
    font-size: 0.8rem;
    color: var(--muted);
  }
</style>

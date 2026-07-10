<script lang="ts">
  export interface TreeFolderNode {
    type: 'folder';
    id: number;
    name: string;
    children: TreeItem[];
  }
  export interface TreeNoteNode {
    type: 'note';
    id: number;
    title: string;
  }
  export type TreeItem = TreeFolderNode | TreeNoteNode;

  export let item: TreeItem;
  export let depth: number;
  export let expandedFolders: Set<number>;
  export let selectedNoteId: number | null;
  export let focusedKey: string | null;
  export let renamingKey: string | null;
  export let onToggleFolder: (id: number) => void;
  export let onSelectNote: (id: number) => void;
  export let onFolderContextMenu: (event: MouseEvent, folderId: number) => void;
  export let onNoteContextMenu: (event: MouseEvent, noteId: number) => void;
  export let onFocusItem: (key: string) => void;
  export let onRenameCommit: (key: string, value: string) => void;
  export let onRenameCancel: () => void;

  let renameCommitted = false;

  $: isExpanded = item.type === 'folder' && expandedFolders.has(item.id);
  $: key = item.type === 'folder' ? `folder:${item.id}` : `note:${item.id}`;
  $: isFocused = focusedKey === key;
  $: isRenaming = renamingKey === key;
  $: if (isRenaming) renameCommitted = false;

  const commitOnce = (value: string) => {
    if (renameCommitted) return;
    renameCommitted = true;
    onRenameCommit(key, value);
  };

  function focusAndSelect(node: HTMLInputElement) {
    node.focus();
    node.select();
  }
</script>

{#if item.type === 'folder'}
  <div
    class="row folder-row"
    class:focused={isFocused}
    style="padding-left: {depth * 14 + 4}px"
    on:click={() => {
      onFocusItem(key);
      onToggleFolder(item.id);
    }}
    on:contextmenu|preventDefault|stopPropagation={(e) => {
      onFocusItem(key);
      onFolderContextMenu(e, item.id);
    }}
    on:keydown={(e) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        onFocusItem(key);
        onToggleFolder(item.id);
      }
    }}
    role="treeitem"
    aria-expanded={isExpanded}
    aria-selected={isFocused}
    tabindex="-1"
  >
    <svg class="chevron" class:open={isExpanded} width="10" height="10" viewBox="0 0 10 10">
      <path d="M3 1 L7 5 L3 9" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
    </svg>
    <svg class="icon folder-icon" width="13" height="13" viewBox="0 0 16 16">
      <path
        fill="currentColor"
        d="M1.5 3A1.5 1.5 0 0 1 3 1.5h3.17a1.5 1.5 0 0 1 1.06.44l.83.82H13A1.5 1.5 0 0 1 14.5 4.26V12.5A1.5 1.5 0 0 1 13 14H3a1.5 1.5 0 0 1-1.5-1.5V3Z"
      />
    </svg>
    {#if isRenaming}
      <input
        class="rename-input"
        value={item.name}
        use:focusAndSelect
        on:click|stopPropagation
        on:keydown={(e) => {
          e.stopPropagation();
          if (e.key === 'Enter') {
            e.preventDefault();
            commitOnce((e.target as HTMLInputElement).value);
          } else if (e.key === 'Escape') {
            e.preventDefault();
            onRenameCancel();
          }
        }}
        on:blur={(e) => commitOnce((e.target as HTMLInputElement).value)}
      />
    {:else}
      <span class="label">{item.name}</span>
    {/if}
  </div>
  {#if isExpanded}
    {#each item.children as child (child.type + ':' + child.id)}
      <svelte:self
        item={child}
        depth={depth + 1}
        {expandedFolders}
        {selectedNoteId}
        {focusedKey}
        {renamingKey}
        {onToggleFolder}
        {onSelectNote}
        {onFolderContextMenu}
        {onNoteContextMenu}
        {onFocusItem}
        {onRenameCommit}
        {onRenameCancel}
      />
    {/each}
  {/if}
{:else}
  <div
    class="row note-row"
    class:selected={selectedNoteId === item.id}
    class:focused={isFocused}
    style="padding-left: {depth * 14 + 20}px"
    on:click={() => {
      onFocusItem(key);
      onSelectNote(item.id);
    }}
    on:contextmenu|preventDefault|stopPropagation={(e) => {
      onFocusItem(key);
      onNoteContextMenu(e, item.id);
    }}
    on:keydown={(e) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        onFocusItem(key);
        onSelectNote(item.id);
      }
    }}
    role="treeitem"
    aria-selected={selectedNoteId === item.id}
    tabindex="-1"
  >
    <svg class="icon note-icon" width="12" height="12" viewBox="0 0 16 16">
      <path
        fill="currentColor"
        d="M4 1.5h5.17a1 1 0 0 1 .7.3l2.83 2.83a1 1 0 0 1 .3.7V14a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V2.5a1 1 0 0 1 1-1Z"
      />
    </svg>
    {#if isRenaming}
      <input
        class="rename-input"
        value={item.title}
        use:focusAndSelect
        on:click|stopPropagation
        on:keydown={(e) => {
          e.stopPropagation();
          if (e.key === 'Enter') {
            e.preventDefault();
            commitOnce((e.target as HTMLInputElement).value);
          } else if (e.key === 'Escape') {
            e.preventDefault();
            onRenameCancel();
          }
        }}
        on:blur={(e) => commitOnce((e.target as HTMLInputElement).value)}
      />
    {:else}
      <span class="label">{item.title || 'Untitled'}</span>
    {/if}
  </div>
{/if}

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.22rem 0.4rem;
    font-size: 0.78rem;
    border-radius: 0.3rem;
    cursor: pointer;
    user-select: none;
    white-space: nowrap;
    overflow: hidden;
  }

  .row:hover {
    background: var(--panel-2);
  }

  .chevron {
    flex-shrink: 0;
    color: var(--muted);
    transition: transform 0.1s ease;
  }

  .chevron.open {
    transform: rotate(90deg);
  }

  .icon {
    flex-shrink: 0;
  }

  .folder-icon {
    color: #e8a33d;
  }

  .note-icon {
    color: var(--muted);
    opacity: 0.85;
  }

  .label {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .rename-input {
    flex: 1;
    min-width: 0;
    font-size: 0.78rem;
    font-family: inherit;
    background: var(--panel);
    color: inherit;
    border: 1px solid var(--accent);
    border-radius: 0.2rem;
    padding: 0 0.2rem;
  }
</style>

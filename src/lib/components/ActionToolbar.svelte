<script lang="ts">
  // Menu-item construction stays in App.svelte (it needs contextMenu state
  // and calls into notesStore actions) - this component just owns the
  // buttons and hands back where to anchor the resulting menu.
  export let isMarkdownActive: boolean;
  export let onOpenNotesMenu: (rect: DOMRect) => void;
  export let onOpenInsertMenu: (rect: DOMRect) => void;
  export let onShowMarkdownHelp: () => void;
  export let onShowShortcuts: () => void;
  export let onShowSettings: () => void;
  // Format and search/replace only exist in editor mode's CodeMirror view -
  // both gated on editorModeEnabled, the same way the Markdown-guide button
  // below is gated on isMarkdownActive.
  export let onFormat: () => void;
  export let editorModeEnabled = false;
  export let onSearch: () => void;
  export let splitViewEnabled = false;
  export let onToggleSplitView: () => void;

  let notesButton: HTMLButtonElement;
  let insertButton: HTMLButtonElement;
</script>

<div class="action-toolbar" on:contextmenu|preventDefault>
  <button class="toolbar-btn" bind:this={notesButton} on:click={() => onOpenNotesMenu(notesButton.getBoundingClientRect())} aria-label="Notes">
    <svg width="11" height="11" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
      <path d="M4 1.5h5.17a1 1 0 0 1 .7.3l2.83 2.83a1 1 0 0 1 .3.7V14a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V2.5a1 1 0 0 1 1-1Z" />
    </svg>
    <span>Notes</span>
    <svg class="caret" width="7" height="7" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
      <path d="M2.5 3.5L5 6.5L7.5 3.5" />
    </svg>
  </button>

  <button class="toolbar-btn" bind:this={insertButton} on:click={() => onOpenInsertMenu(insertButton.getBoundingClientRect())} aria-label="Insert">
    <svg width="10" height="10" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
      <path d="M8 3v10M3 8h10" />
    </svg>
    <span>Insert</span>
    <svg class="caret" width="7" height="7" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
      <path d="M2.5 3.5L5 6.5L7.5 3.5" />
    </svg>
  </button>

  <div class="toolbar-right">
    {#if editorModeEnabled}
      <button class="toolbar-btn" on:click={onFormat} aria-label="Format (Alt+F)" title="Format (Alt+F)">
        <svg width="11" height="11" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
          <path d="M2 3h12M2 8h8M2 13h5" />
        </svg>
        <span>Format</span>
      </button>

      <button class="toolbar-btn" on:click={onSearch} aria-label="Search (Ctrl/Cmd+F)" title="Search (Ctrl/Cmd+F)">
        <svg width="11" height="11" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="7" cy="7" r="4.5" />
          <path d="M10.3 10.3L14 14" />
        </svg>
        <span>Search</span>
      </button>
    {/if}

    {#if isMarkdownActive}
      <button class="toolbar-btn" on:click={onShowMarkdownHelp} aria-label="Markdown guide">
        <svg width="11" height="11" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
          <path d="M2 4h5M2 8h5M2 12h3" />
          <path d="M10.5 3.5v9M10.5 3.5l2 2.5 2-2.5M14.5 8.5l-2 2.5-2-2.5" />
        </svg>
        <span>Markdown</span>
      </button>
    {/if}

    <button
      class="toolbar-btn"
      class:active={splitViewEnabled}
      on:click={onToggleSplitView}
      aria-label="Split view (Alt+V)"
      aria-pressed={splitViewEnabled}
      title="Split view (Alt+V)"
    >
      <svg width="11" height="11" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
        <rect x="1.5" y="2.5" width="13" height="11" rx="1.2" />
        <path d="M8 2.5v11" />
      </svg>
      <span>Split</span>
    </button>

    <button class="toolbar-btn" on:click={onShowShortcuts} aria-label="Keyboard shortcuts">
      <svg width="11" height="11" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round">
        <circle cx="8" cy="8" r="6.5" />
        <path d="M6.1 6.2a1.9 1.9 0 1 1 2.7 1.7c-.7.35-.9.7-.9 1.4" stroke-linejoin="round" />
        <circle cx="8" cy="11.4" r="0.15" fill="currentColor" />
      </svg>
      <span>Shortcuts</span>
    </button>

    <button class="toolbar-btn" on:click={onShowSettings} aria-label="Settings">
      <svg width="11" height="11" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="8" cy="8" r="2.2" />
        <path d="M8 2v1.6M8 12.4V14M14 8h-1.6M3.6 8H2M12.13 3.87l-1.13 1.13M4.99 11.01l-1.13 1.13M12.13 12.13l-1.13-1.13M4.99 4.99 3.87 3.87" />
      </svg>
      <span>Settings</span>
    </button>
  </div>
</div>

<style>
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

  .toolbar-btn.active,
  .toolbar-btn.active:hover {
    background: var(--panel-2);
    color: var(--accent);
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
</style>

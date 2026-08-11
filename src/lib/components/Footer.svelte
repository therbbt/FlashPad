<script lang="ts">
  // Search-scope toggle and status text talk to the database/status stores
  // directly (they're simple, single-purpose, already-global state) - only
  // the values that mix local editor-mirror state with notes/database data
  // (computed in App.svelte, since the sidebar tree needs them too) come in
  // as props.
  import { hasSearchableOtherSources, searchAllDatabases, toggleSearchAllDatabases } from '../stores/databaseStore';
  import { selectedId } from '../stores/notesStore';
  import { status } from '../stores/statusStore';
  import { vimModeIndicator } from '../stores/vimModeIndicator';

  export let query: string;
  export let isSearching: boolean;
  export let searchResultsCount: number;
  export let searchMatchIndex: number;
  export let isMarkdownActive: boolean;
  export let isLockedActive: boolean;
  export let onSearchKeydown: (event: KeyboardEvent) => void;
  export let onGoToSearchMatch: (direction: 1 | -1) => void;
  export let onToggleMarkdown: () => void;
</script>

<footer class="footer" on:contextmenu|preventDefault>
  <div class="search-box">
    <div class="search-input-wrap">
      <input
        class="search-input"
        class:with-scope-btn={$hasSearchableOtherSources}
        bind:value={query}
        on:keydown={onSearchKeydown}
        placeholder="Search notes"
      />
      {#if $hasSearchableOtherSources}
        <button
          class="search-scope-btn"
          class:active={$searchAllDatabases}
          on:click={toggleSearchAllDatabases}
          aria-pressed={$searchAllDatabases}
          aria-label="Search all databases"
          title="Search all databases"
        >
          <svg width="15" height="15" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="8" cy="8" r="6.2" />
            <ellipse cx="8" cy="8" rx="2.6" ry="6.2" />
            <path d="M1.9 5.8h12.2M1.9 10.2h12.2" />
          </svg>
        </button>
      {/if}
    </div>
    {#if isSearching}
      <span class="search-count">{searchResultsCount ? `${searchMatchIndex + 1}/${searchResultsCount}` : '0/0'}</span>
      <button
        class="search-nav-btn"
        on:click={() => onGoToSearchMatch(-1)}
        disabled={!searchResultsCount}
        aria-label="Previous match"
      >
        <svg width="8" height="8" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
          <path d="M2.5 6.5L5 3.5L7.5 6.5" />
        </svg>
      </button>
      <button
        class="search-nav-btn"
        on:click={() => onGoToSearchMatch(1)}
        disabled={!searchResultsCount}
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
    on:click={onToggleMarkdown}
    disabled={$selectedId == null || isLockedActive}
    aria-pressed={isMarkdownActive}
  >
    Markdown
  </button>
  <div class="footer-right">
    {#if $vimModeIndicator}
      <span class="vim-mode-badge">{$vimModeIndicator}</span>
    {/if}
    <span class="status">{$status}</span>
  </div>
</footer>

<style>
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

  .search-input-wrap {
    position: relative;
    display: flex;
    align-items: center;
    width: 200px;
    flex-shrink: 0;
  }

  .footer .search-input {
    width: 100%;
    border: 1px solid var(--border);
    border-radius: 0.5rem;
    background: var(--panel-2);
    color: inherit;
    font-size: 0.8rem;
    padding: 0.35rem 0.6rem;
  }

  .footer .search-input.with-scope-btn {
    padding-right: 1.95rem;
  }

  .search-count {
    font-size: 0.75rem;
    color: var(--muted);
    min-width: 2.5rem;
    text-align: center;
  }

  .footer-right {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    margin-left: auto;
  }

  .status {
    font-size: 0.72rem;
    color: var(--muted);
    white-space: nowrap;
  }

  .vim-mode-badge {
    font-size: 0.68rem;
    font-weight: 600;
    letter-spacing: 0.02em;
    color: var(--md-color, #4dd0c8);
    background: var(--panel-2);
    border: 1px solid var(--border);
    border-radius: 0.3rem;
    padding: 0.1rem 0.4rem;
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

  .search-scope-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    position: absolute;
    right: 0.25rem;
    top: 50%;
    transform: translateY(-50%);
    width: 1.5rem;
    height: 1.5rem;
    border: none;
    border-radius: 0.3rem;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
  }

  .search-scope-btn:hover {
    color: var(--text);
  }

  .search-scope-btn.active,
  .search-scope-btn.active:hover {
    background: rgba(77, 208, 200, 0.16);
    color: #4dd0c8;
  }
</style>

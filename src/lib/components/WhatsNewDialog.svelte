<script lang="ts">
  import { openUrl } from '@tauri-apps/plugin-opener';
  import MarkdownEditor from './MarkdownEditor.svelte';
  import { isAllowedLinkUrl } from '../utils/links';
  import type { ReleaseNotes } from '../services/releaseNotesService';

  export let notes: ReleaseNotes;
  export let onClose: () => void;

  let panelEl: HTMLDivElement;

  // Release notes are read-only, but a link in them should still open -
  // same protocol check as the notes editor's openLink, just without a
  // toast for this lower-stakes surface.
  const openNotesLink = (url: string) => {
    if (!isAllowedLinkUrl(url)) return;
    void openUrl(url).catch(() => {});
  };

  const formattedDate = (() => {
    if (!notes.publishedAt) return null;
    const parsed = new Date(notes.publishedAt);
    if (Number.isNaN(parsed.getTime())) return null;
    return parsed.toLocaleDateString(undefined, { year: 'numeric', month: 'long', day: 'numeric' });
  })();

  const handleKeydown = (event: KeyboardEvent) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      onClose();
    }
  };

  const handleOutsideClick = (event: MouseEvent) => {
    if (panelEl && !panelEl.contains(event.target as Node)) {
      onClose();
    }
  };
</script>

<svelte:window on:keydown={handleKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="overlay" on:mousedown={handleOutsideClick}>
  <div class="panel" bind:this={panelEl} role="dialog" aria-modal="true" aria-label="What's new">
    <header>
      <h2>What's new in FlashPad {notes.version}</h2>
      <button class="close" on:click={onClose} aria-label="Close">
        <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
          <path d="M2 2l12 12M14 2L2 14" />
        </svg>
      </button>
    </header>

    {#if formattedDate}
      <p class="date">Released {formattedDate}</p>
    {/if}

    {#if notes.body}
      <div class="notes">
        <MarkdownEditor content={notes.body} noteId={0} onUpdate={() => {}} onOpenLink={openNotesLink} editable={false} />
      </div>
    {:else}
      <p class="notes empty">No release notes provided.</p>
    {/if}

    <div class="actions">
      <button class="btn primary" on:click={onClose}>Got it</button>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: var(--window-shadow-margin, 0);
    background: rgba(0, 0, 0, 0.35);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1200;
  }

  .panel {
    width: min(420px, 90vw);
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 0.6rem;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.4);
    padding: 0.85rem;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    margin-bottom: 0.5rem;
  }

  h2 {
    margin: 0;
    font-size: 0.9rem;
  }

  .close {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    width: 1.5rem;
    height: 1.5rem;
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    background: var(--panel-2);
    color: var(--muted);
    padding: 0;
  }

  .close:hover {
    color: var(--text);
  }

  .date {
    margin: 0 0 0.6rem;
    font-size: 0.72rem;
    color: var(--muted);
  }

  .notes {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 3rem;
    max-height: 40vh;
    overflow: auto;
    background: var(--panel-2);
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    font-size: 0.78rem;
    color: var(--text);
    margin: 0 0 0.6rem;
  }

  .notes :global(.markdown-editor .tiptap) {
    padding: 0.6rem 0.7rem;
  }

  p.notes.empty {
    padding: 0.6rem 0.7rem;
    color: var(--muted);
  }

  .actions {
    display: flex;
    justify-content: flex-end;
  }

  .btn {
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    background: var(--panel-2);
    color: var(--text);
    font-size: 0.8rem;
    padding: 0.4rem 0.85rem;
    cursor: pointer;
  }

  .btn.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }

  .btn.primary:hover {
    filter: brightness(1.1);
  }
</style>

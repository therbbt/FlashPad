<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { writeText as writeClipboardText } from '@tauri-apps/plugin-clipboard-manager';

  export let createdAt: string | null;
  export let updatedAt: string | null;
  export let onError: (message: string) => void;

  let open = false;
  let copiedField: 'created' | 'updated' | null = null;
  let copiedFieldTimer: ReturnType<typeof setTimeout> | undefined;

  const toggle = () => {
    open = !open;
  };

  const handleOutsideClick = (event: MouseEvent) => {
    const target = event.target as HTMLElement;
    if (!target.closest('.note-info')) {
      open = false;
    }
  };

  // Writes to the real OS clipboard (via tauri-plugin-clipboard-manager) -
  // briefly swaps the clicked field's copy icon for a checkmark instead of
  // routing through the app's footer status text, since the popover is
  // already showing the value right there - no need to look away to
  // confirm it copied.
  const copyField = async (field: 'created' | 'updated', value: string) => {
    try {
      await writeClipboardText(value.replace('T', ' '));
      copiedField = field;
      if (copiedFieldTimer) clearTimeout(copiedFieldTimer);
      copiedFieldTimer = setTimeout(() => (copiedField = null), 1200);
    } catch (err) {
      onError(err instanceof Error ? err.message : 'Failed to copy');
    }
  };

  onMount(() => {
    window.addEventListener('mousedown', handleOutsideClick);
  });

  onDestroy(() => {
    window.removeEventListener('mousedown', handleOutsideClick);
  });
</script>

<div class="note-info">
  <button
    class="info-btn"
    type="button"
    on:click|stopPropagation={toggle}
    aria-haspopup="dialog"
    aria-expanded={open}
    aria-label="Note info"
  >
    <svg width="13" height="13" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round">
      <circle cx="8" cy="8" r="6.5" />
      <path d="M8 7.2v4.3" />
      <circle cx="8" cy="4.7" r="0.15" fill="currentColor" />
    </svg>
  </button>
  {#if open}
    <div class="note-info-popover">
      {#if createdAt}
        <div class="note-info-row">
          <div class="note-info-text">
            <span class="note-info-label">Created</span>
            <span class="note-info-value">{createdAt.replace('T', ' ')}</span>
          </div>
          <button
            class="note-info-copy"
            type="button"
            on:click={() => createdAt && copyField('created', createdAt)}
            aria-label="Copy created date"
          >
            {#if copiedField === 'created'}
              <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
                <path d="M3 8.5L6.5 12L13 4.5" />
              </svg>
            {:else}
              <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
                <rect x="5.5" y="5.5" width="8" height="8" rx="1" />
                <path d="M3 10.5V3.5a1 1 0 0 1 1-1H10" />
              </svg>
            {/if}
          </button>
        </div>
      {/if}
      {#if updatedAt}
        <div class="note-info-row">
          <div class="note-info-text">
            <span class="note-info-label">Updated</span>
            <span class="note-info-value">{updatedAt.replace('T', ' ')}</span>
          </div>
          <button
            class="note-info-copy"
            type="button"
            on:click={() => updatedAt && copyField('updated', updatedAt)}
            aria-label="Copy updated date"
          >
            {#if copiedField === 'updated'}
              <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
                <path d="M3 8.5L6.5 12L13 4.5" />
              </svg>
            {:else}
              <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
                <rect x="5.5" y="5.5" width="8" height="8" rx="1" />
                <path d="M3 10.5V3.5a1 1 0 0 1 1-1H10" />
              </svg>
            {/if}
          </button>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .note-info {
    position: relative;
  }

  .info-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 1.4rem;
    height: 1.4rem;
    border: none;
    border-radius: 50%;
    background: transparent;
    color: var(--muted);
    padding: 0;
  }

  .info-btn:hover,
  .info-btn[aria-expanded='true'] {
    background: var(--panel-2);
    color: var(--text);
  }

  .note-info-popover {
    position: absolute;
    top: calc(100% + 0.35rem);
    right: 0;
    z-index: 10;
    width: max-content;
    min-width: 11rem;
    background: var(--panel-2);
    border: 1px solid var(--border);
    border-radius: 0.5rem;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
    padding: 0.4rem;
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }

  .note-info-row {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.3rem 0.4rem;
    border-radius: 0.35rem;
  }

  .note-info-row:hover {
    background: var(--panel);
  }

  .note-info-text {
    display: flex;
    flex-direction: column;
    gap: 0.05rem;
    min-width: 0;
    margin-right: auto;
  }

  .note-info-label {
    font-size: 0.65rem;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    color: var(--muted);
  }

  .note-info-value {
    font-size: 0.76rem;
    color: var(--text);
    white-space: nowrap;
  }

  .note-info-copy {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 1.4rem;
    height: 1.4rem;
    border: 1px solid var(--border);
    border-radius: 0.35rem;
    background: var(--panel);
    color: var(--muted);
    padding: 0;
  }

  .note-info-copy:hover {
    color: var(--accent);
    border-color: var(--accent);
  }
</style>

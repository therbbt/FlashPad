<script lang="ts">
  import type { Update } from '@tauri-apps/plugin-updater';
  import { relaunch } from '@tauri-apps/plugin-process';

  export let update: Update;
  // Covers both "Not now" and closing the dialog any other way (X, Escape,
  // outside click) - all count as the user having seen and passed on this
  // version, so the reminder doesn't reappear until a newer one ships.
  export let onDismiss: () => void;

  let panelEl: HTMLDivElement;
  let installing = false;
  let progressLabel = '';
  let errorMessage = '';

  const formattedDate = (() => {
    if (!update.date) return null;
    const parsed = new Date(update.date);
    if (Number.isNaN(parsed.getTime())) return null;
    return parsed.toLocaleDateString(undefined, { year: 'numeric', month: 'long', day: 'numeric' });
  })();

  const handleKeydown = (event: KeyboardEvent) => {
    if (event.key === 'Escape' && !installing) {
      event.preventDefault();
      onDismiss();
    }
  };

  const handleOutsideClick = (event: MouseEvent) => {
    if (installing) return;
    if (panelEl && !panelEl.contains(event.target as Node)) {
      onDismiss();
    }
  };

  const install = async () => {
    installing = true;
    errorMessage = '';
    progressLabel = 'Downloading update…';
    let totalBytes = 0;
    let downloadedBytes = 0;
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === 'Started') {
          totalBytes = event.data.contentLength ?? 0;
        } else if (event.event === 'Progress') {
          downloadedBytes += event.data.chunkLength;
          progressLabel =
            totalBytes > 0
              ? `Downloading update… ${Math.min(100, Math.round((downloadedBytes / totalBytes) * 100))}%`
              : 'Downloading update…';
        } else if (event.event === 'Finished') {
          progressLabel = 'Installing…';
        }
      });
      await relaunch();
    } catch (err) {
      installing = false;
      errorMessage = err instanceof Error ? err.message : 'Failed to install the update. Please try again later.';
    }
  };
</script>

<svelte:window on:keydown={handleKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="overlay" on:mousedown={handleOutsideClick}>
  <div class="panel" bind:this={panelEl} role="dialog" aria-modal="true" aria-label="Update available">
    <header>
      <h2>Update available</h2>
      <button class="close" on:click={onDismiss} disabled={installing} aria-label="Close">
        <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
          <path d="M2 2l12 12M14 2L2 14" />
        </svg>
      </button>
    </header>

    <p class="version-line">
      <strong>FlashPad {update.version}</strong>
      <span class="muted">you have {update.currentVersion}</span>
    </p>
    {#if formattedDate}
      <p class="date">Released {formattedDate}</p>
    {/if}

    {#if update.body}
      <div class="notes">{update.body}</div>
    {:else}
      <p class="notes empty">No release notes provided.</p>
    {/if}

    {#if errorMessage}
      <p class="error">{errorMessage}</p>
    {/if}

    <div class="actions">
      <button class="btn" on:click={onDismiss} disabled={installing}>Not now</button>
      <button class="btn primary" on:click={install} disabled={installing}>
        {installing ? progressLabel : 'Install and Restart'}
      </button>
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
    width: 1.5rem;
    height: 1.5rem;
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    background: var(--panel-2);
    color: var(--muted);
    padding: 0;
  }

  .close:hover:not(:disabled) {
    color: var(--text);
  }

  .close:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .version-line {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
    margin: 0 0 0.15rem;
    font-size: 0.85rem;
    color: var(--text);
  }

  .muted {
    font-size: 0.72rem;
    color: var(--muted);
  }

  .date {
    margin: 0 0 0.6rem;
    font-size: 0.72rem;
    color: var(--muted);
  }

  .notes {
    flex: 1;
    min-height: 3rem;
    max-height: 40vh;
    overflow: auto;
    white-space: pre-wrap;
    word-wrap: break-word;
    background: var(--panel-2);
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    padding: 0.6rem 0.7rem;
    font-size: 0.78rem;
    line-height: 1.5;
    color: var(--text);
    margin: 0 0 0.6rem;
  }

  .notes.empty {
    color: var(--muted);
  }

  .error {
    margin: 0 0 0.6rem;
    font-size: 0.75rem;
    color: #ef4444;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
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

  .btn:hover:not(:disabled) {
    background: var(--border);
  }

  .btn:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .btn.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }

  .btn.primary:hover:not(:disabled) {
    filter: brightness(1.1);
  }
</style>

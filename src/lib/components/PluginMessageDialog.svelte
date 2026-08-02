<script lang="ts">
  // Small dismissable popup for the plugin API's ui.showMessage() - the
  // shell mirrors PluginFormDialog.svelte/ConfirmDialog.svelte.
  import { openUrl } from '@tauri-apps/plugin-opener';

  export let title: string;
  export let message: string;
  export let linkLabel: string | undefined = undefined;
  export let linkUrl: string | undefined = undefined;
  export let onClose: () => void;

  let panelEl: HTMLDivElement;
  let openError = '';

  const openLink = async () => {
    if (!linkUrl) return;
    openError = '';
    try {
      await openUrl(linkUrl);
    } catch (err) {
      openError = err instanceof Error ? err.message : 'Failed to open link';
    }
  };

  const handleKeydown = (event: KeyboardEvent) => {
    if (event.key === 'Escape' || event.key === 'Enter') {
      event.preventDefault();
      onClose();
    }
  };

  const handleOutsideClick = (event: MouseEvent) => {
    if (panelEl && !panelEl.contains(event.target as Node)) onClose();
  };
</script>

<svelte:window on:keydown={handleKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="overlay" on:mousedown={handleOutsideClick}>
  <div class="panel" bind:this={panelEl} role="dialog" aria-modal="true" aria-label={title}>
    <h2 class="title">{title}</h2>
    <p class="message">{message}</p>
    {#if openError}
      <p class="error">{openError}</p>
    {/if}
    <div class="actions">
      {#if linkUrl}
        <button class="btn" on:click={openLink}>{linkLabel ?? 'Open link'}</button>
      {/if}
      <button class="btn primary" on:click={onClose}>Close</button>
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
    width: min(360px, 90vw);
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 0.6rem;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.4);
    padding: 1rem;
  }

  .title {
    margin: 0 0 0.6rem;
    font-size: 0.9rem;
    color: var(--text);
  }

  .message {
    margin: 0 0 1rem;
    font-size: 0.8rem;
    line-height: 1.4;
    color: var(--muted);
    white-space: pre-wrap;
  }

  .error {
    margin: -0.5rem 0 1rem;
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
    padding: 0.35rem 0.85rem;
    cursor: pointer;
  }

  .btn:hover {
    background: var(--border);
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

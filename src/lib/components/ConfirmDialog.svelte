<script lang="ts">
  export let message: string;
  export let confirmLabel = 'Delete';
  export let onConfirm: () => void;
  export let onCancel: () => void;

  let panelEl: HTMLDivElement;

  const handleKeydown = (event: KeyboardEvent) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      onCancel();
    } else if (event.key === 'Enter') {
      event.preventDefault();
      onConfirm();
    }
  };

  const handleOutsideClick = (event: MouseEvent) => {
    if (panelEl && !panelEl.contains(event.target as Node)) {
      onCancel();
    }
  };
</script>

<svelte:window on:keydown={handleKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="overlay" on:mousedown={handleOutsideClick}>
  <div class="panel" bind:this={panelEl} role="alertdialog" aria-modal="true" aria-label="Confirm">
    <p class="message">{message}</p>
    <div class="actions">
      <button class="btn" on:click={onCancel}>Cancel</button>
      <button class="btn danger" on:click={onConfirm}>{confirmLabel}</button>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.35);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1200;
  }

  .panel {
    width: min(320px, 90vw);
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 0.6rem;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.4);
    padding: 1rem;
  }

  .message {
    margin: 0 0 1rem;
    font-size: 0.82rem;
    color: var(--text);
    line-height: 1.4;
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

  .btn.danger {
    background: #ef4444;
    border-color: #ef4444;
    color: #fff;
  }

  .btn.danger:hover {
    background: #dc2626;
    border-color: #dc2626;
  }
</style>

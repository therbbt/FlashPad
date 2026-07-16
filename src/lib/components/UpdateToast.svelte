<script lang="ts">
  export let version: string;
  export let onViewDetails: () => void;
  export let onDismiss: () => void;
</script>

<div class="toast" role="status">
  <div class="icon">
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
      <path d="M8 2v7.5M8 9.5L5 6.5M8 9.5l3-3" />
      <path d="M2.5 11v2a1 1 0 0 0 1 1h9a1 1 0 0 0 1-1v-2" />
    </svg>
  </div>
  <div class="body">
    <span class="title">Update available</span>
    <span class="subtitle">FlashPad {version} is ready to install</span>
  </div>
  <button class="link" on:click={onViewDetails}>What's new</button>
  <button class="close" on:click={onDismiss} aria-label="Dismiss">
    <svg width="10" height="10" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
      <path d="M2 2l12 12M14 2L2 14" />
    </svg>
  </button>
</div>

<style>
  .toast {
    position: fixed;
    right: calc(var(--window-shadow-margin, 0px) + 0.75rem);
    bottom: calc(var(--window-shadow-margin, 0px) + 0.75rem);
    z-index: 1300;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    max-width: min(320px, calc(100vw - 1.5rem));
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 0.6rem;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.4);
    padding: 0.55rem 0.6rem;
  }

  .icon {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    width: 1.6rem;
    height: 1.6rem;
    border-radius: 50%;
    background: var(--accent-soft, var(--panel-2));
    color: var(--accent);
  }

  .body {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    min-width: 0;
    margin-right: auto;
  }

  .title {
    font-size: 0.78rem;
    font-weight: 600;
    color: var(--text);
  }

  .subtitle {
    font-size: 0.7rem;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .link {
    flex-shrink: 0;
    background: none;
    border: none;
    color: var(--accent);
    font-size: 0.75rem;
    font-weight: 600;
    cursor: pointer;
    padding: 0.2rem;
  }

  .link:hover {
    text-decoration: underline;
  }

  .close {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    width: 1.3rem;
    height: 1.3rem;
    border: none;
    border-radius: 0.35rem;
    background: transparent;
    color: var(--muted);
    padding: 0;
  }

  .close:hover {
    background: var(--panel-2);
    color: var(--text);
  }
</style>

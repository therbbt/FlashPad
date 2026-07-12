<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';

  const isTauriRuntime = () => typeof window !== 'undefined' && Boolean((window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);

  let isMaximized = false;
  let unlisten: (() => void) | undefined;

  onMount(async () => {
    if (!isTauriRuntime()) return;
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    const win = getCurrentWindow();
    isMaximized = await win.isMaximized();
    unlisten = await win.onResized(async () => {
      isMaximized = await win.isMaximized();
    });
  });

  onDestroy(() => unlisten?.());

  const withWindow = async (fn: (win: import('@tauri-apps/api/window').Window) => Promise<void>) => {
    if (!isTauriRuntime()) return;
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    await fn(getCurrentWindow());
  };

  // Minimize and close both just hide to the tray, matching the Escape key
  // and the global hotkey - this app has no taskbar-minimized state, it's
  // either open or living in the tray. Routed through the same `hide_window`
  // command Escape uses (rather than calling the window API's hide()
  // directly) so Rust's own shown/hidden tracking - which the hotkey relies
  // on - stays in sync no matter which of these triggers the hide.
  const hide = () => {
    if (!isTauriRuntime()) return;
    void invoke('hide_window');
  };
  const minimize = hide;
  const toggleMaximize = () => void withWindow((win) => win.toggleMaximize());
  const close = hide;
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="title-bar" data-tauri-drag-region on:dblclick={toggleMaximize}>
  <div class="brand">
    <svg class="brand-icon" width="22" height="22" viewBox="0 0 48 48" fill="none" stroke="currentColor" stroke-width="3.6" stroke-linecap="round" stroke-linejoin="round">
      <circle cx="24" cy="25" r="11.5" />
      <path d="M24 13.5v4.5" />
      <path d="M18.5 25.5l4.2 4.2 7-8.4" />
    </svg>
    <span class="brand-name">FlashPad</span>
  </div>
  <div class="controls">
    <button class="control" on:click={minimize} aria-label="Minimize">
      <svg width="10" height="10" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round">
        <path d="M3 8h10" />
      </svg>
    </button>
    <button class="control" on:click={toggleMaximize} aria-label={isMaximized ? 'Restore' : 'Maximize'}>
      {#if isMaximized}
        <svg width="10" height="10" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round">
          <path d="M5.5 3.5h6a1 1 0 0 1 1 1v6" />
          <rect x="3.5" y="5.5" width="7" height="7" rx="0.8" />
        </svg>
      {:else}
        <svg width="10" height="10" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round">
          <rect x="3.5" y="3.5" width="9" height="9" rx="0.8" />
        </svg>
      {/if}
    </button>
    <button class="control close" on:click={close} aria-label="Close">
      <svg width="10" height="10" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round">
        <path d="M3 3l10 10M13 3l-10 10" />
      </svg>
    </button>
  </div>
</div>

<style>
  .title-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-shrink: 0;
    height: 28px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    user-select: none;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding-left: 0.6rem;
    color: var(--text);
    overflow: hidden;
  }

  .brand-icon {
    flex-shrink: 0;
  }

  /* Same swap as the sidebar: the mark reads as the opposite panel shade -
     white on dark theme, dark on light theme - rather than the accent blue.
     Dimmed on dark theme specifically since full white was too bright/loud
     against the dark title bar. */
  :global(html:not([data-theme='light'])) .brand-icon {
    color: rgba(255, 255, 255, 0.55);
  }

  :global(html[data-theme='light']) .brand-icon {
    color: #211d18;
  }

  .brand-name {
    font-size: 0.72rem;
    font-weight: 500;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .controls {
    display: flex;
    align-items: stretch;
    height: 100%;
  }

  .control {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 40px;
    height: 100%;
    border: 0;
    background: transparent;
    color: var(--muted);
    padding: 0;
  }

  .control:hover {
    background: var(--panel-2);
    color: var(--text);
  }

  .control.close:hover {
    background: #ef4444;
    color: #fff;
  }
</style>

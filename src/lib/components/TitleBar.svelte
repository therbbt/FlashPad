<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';

  const isTauriRuntime = () => typeof window !== 'undefined' && Boolean((window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);

  // Rounded corners + drop shadow read as "floating" when the window is
  // free-floating, but look wrong once it's flush against the screen edges
  // (maximized, fullscreen, or snapped to one half via the WM's own
  // tiling) - App.svelte squares the shell's corners off while this is true.
  export let squared = false;

  let isMaximized = false;
  let unlistenResize: (() => void) | undefined;
  let unlistenMove: (() => void) | undefined;

  // How close (in physical px) an edge has to be to the monitor's own edge
  // to count as "flush" - a couple px of slack for WM/compositor rounding.
  const EDGE_TOLERANCE = 3;

  async function updateSquared(win: import('@tauri-apps/api/window').Window) {
    // currentMonitor() is a module-level function (there's no Window#
    // method for it) - it still reports the monitor of whichever window
    // this script is running in, which is exactly `win` here.
    const { currentMonitor } = await import('@tauri-apps/api/window');
    const [maximized, monitor, position, size] = await Promise.all([
      win.isMaximized(),
      currentMonitor(),
      win.outerPosition(),
      win.outerSize(),
    ]);
    isMaximized = maximized;
    if (!monitor) {
      squared = maximized;
      return;
    }
    // A WM-driven tile (half-screen snap, or one quadrant of a 2x2 grid of
    // windows) isn't reported as "maximized", but it does sit flush against
    // at least one horizontal AND one vertical screen edge - e.g. a
    // top-left quarter tile touches the top and left edges even though it
    // covers neither the full width nor the full height. A window free-
    // floating in the middle of the screen touches neither axis, so it
    // keeps its rounded corners/shadow as normal.
    const touchesLeft = position.x <= monitor.position.x + EDGE_TOLERANCE;
    const touchesRight = position.x + size.width >= monitor.position.x + monitor.size.width - EDGE_TOLERANCE;
    const touchesTop = position.y <= monitor.position.y + EDGE_TOLERANCE;
    const touchesBottom = position.y + size.height >= monitor.position.y + monitor.size.height - EDGE_TOLERANCE;
    squared = maximized || ((touchesLeft || touchesRight) && (touchesTop || touchesBottom));
  }

  onMount(async () => {
    if (!isTauriRuntime()) return;
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    const win = getCurrentWindow();
    await updateSquared(win);
    unlistenResize = await win.onResized(() => void updateSquared(win));
    unlistenMove = await win.onMoved(() => void updateSquared(win));
  });

  onDestroy(() => {
    unlistenResize?.();
    unlistenMove?.();
  });

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
    <svg class="brand-icon" width="14" height="14" viewBox="0 0 48 48" fill="none" stroke="currentColor" stroke-width="4.6" stroke-linecap="round" stroke-linejoin="round">
      <circle cx="24" cy="25" r="15.53" />
      <path d="M32.78 8.13 L18.6 26.35 H24.68 L16.58 41.88" />
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

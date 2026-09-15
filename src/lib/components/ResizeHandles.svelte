<script lang="ts">
  type Direction = 'East' | 'North' | 'NorthEast' | 'NorthWest' | 'South' | 'SouthEast' | 'SouthWest' | 'West';

  // When the window is floating (not squared), the visible rounded shell
  // sits var(--window-shadow-margin) in from the true window edges - these
  // hit zones need to span that whole gap, not just a few px right at the
  // true edge, or there's a dead band you can't grab to resize from.
  // Squared (maximized/snapped) collapses back to a normal thin edge since
  // the shell is flush with the window and there's no gap to span.
  export let squared = false;

  const isTauriRuntime = () => typeof window !== 'undefined' && Boolean((window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);

  const startResize = (direction: Direction) => async (event: MouseEvent) => {
    if (event.button !== 0 || !isTauriRuntime()) return;
    event.preventDefault();
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    await getCurrentWindow().startResizeDragging(direction);
  };

  $: if (typeof document !== 'undefined') {
    document.documentElement.style.setProperty('--resize-handle-thickness', squared ? '4px' : '16px');
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="edge top" on:mousedown={startResize('North')}></div>
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="edge bottom" on:mousedown={startResize('South')}></div>
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="edge left" on:mousedown={startResize('West')}></div>
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="edge right" on:mousedown={startResize('East')}></div>
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="corner top-left" on:mousedown={startResize('NorthWest')}></div>
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="corner top-right" on:mousedown={startResize('NorthEast')}></div>
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="corner bottom-left" on:mousedown={startResize('SouthWest')}></div>
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="corner bottom-right" on:mousedown={startResize('SouthEast')}></div>

<style>
  .edge,
  .corner {
    position: fixed;
    z-index: 2000;
  }

  .edge.top {
    top: 0;
    left: var(--resize-handle-thickness, 4px);
    right: var(--resize-handle-thickness, 4px);
    height: var(--resize-handle-thickness, 4px);
    cursor: ns-resize;
  }

  .edge.bottom {
    bottom: 0;
    left: var(--resize-handle-thickness, 4px);
    right: var(--resize-handle-thickness, 4px);
    height: var(--resize-handle-thickness, 4px);
    cursor: ns-resize;
  }

  .edge.left {
    top: var(--resize-handle-thickness, 4px);
    bottom: var(--resize-handle-thickness, 4px);
    left: 0;
    width: var(--resize-handle-thickness, 4px);
    cursor: ew-resize;
  }

  .edge.right {
    top: var(--resize-handle-thickness, 4px);
    bottom: var(--resize-handle-thickness, 4px);
    right: 0;
    width: var(--resize-handle-thickness, 4px);
    cursor: ew-resize;
  }

  .corner {
    width: var(--resize-handle-thickness, 4px);
    height: var(--resize-handle-thickness, 4px);
  }

  .corner.top-left {
    top: 0;
    left: 0;
    cursor: nwse-resize;
  }

  .corner.top-right {
    top: 0;
    right: 0;
    cursor: nesw-resize;
  }

  .corner.bottom-left {
    bottom: 0;
    left: 0;
    cursor: nesw-resize;
  }

  .corner.bottom-right {
    bottom: 0;
    right: 0;
    cursor: nwse-resize;
  }
</style>

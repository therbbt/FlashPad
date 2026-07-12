<script lang="ts">
  type Direction = 'East' | 'North' | 'NorthEast' | 'NorthWest' | 'South' | 'SouthEast' | 'SouthWest' | 'West';

  const isTauriRuntime = () => typeof window !== 'undefined' && Boolean((window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);

  const startResize = (direction: Direction) => async (event: MouseEvent) => {
    if (event.button !== 0 || !isTauriRuntime()) return;
    event.preventDefault();
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    await getCurrentWindow().startResizeDragging(direction);
  };
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
    left: 6px;
    right: 6px;
    height: 4px;
    cursor: ns-resize;
  }

  .edge.bottom {
    bottom: 0;
    left: 6px;
    right: 6px;
    height: 4px;
    cursor: ns-resize;
  }

  .edge.left {
    top: 6px;
    bottom: 6px;
    left: 0;
    width: 4px;
    cursor: ew-resize;
  }

  .edge.right {
    top: 6px;
    bottom: 6px;
    right: 0;
    width: 4px;
    cursor: ew-resize;
  }

  .corner {
    width: 6px;
    height: 6px;
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

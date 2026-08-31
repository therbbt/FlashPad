<script lang="ts">
  import { onMount } from 'svelte';

  // Drag lifecycle mirrors SidebarResizer.svelte exactly (mousedown ->
  // window-level mousemove/mouseup -> a body class toggled during drag ->
  // persisted value) - but NOT its position math: SidebarResizer computes
  // width straight from clientX, which only works because the sidebar is
  // pinned to the window's left edge. This resizer sits between two panes
  // that can themselves be anywhere, so it computes a 0-1 ratio relative to
  // its own parent's (the .panes row's) bounding rect instead.
  const SPLIT_RATIO_KEY = 'flashpad.splitRatio';
  const MIN_RATIO = 0.2;
  const MAX_RATIO = 0.8;
  const DEFAULT_RATIO = 0.5;

  export let ratio = DEFAULT_RATIO;

  let isResizing = false;
  let resizerEl: HTMLDivElement;

  const loadRatio = (): number => {
    if (typeof window === 'undefined') return DEFAULT_RATIO;
    const raw = Number(window.localStorage.getItem(SPLIT_RATIO_KEY));
    if (!raw || Number.isNaN(raw)) return DEFAULT_RATIO;
    return Math.min(MAX_RATIO, Math.max(MIN_RATIO, raw));
  };

  const saveRatio = () => {
    if (typeof window !== 'undefined') {
      window.localStorage.setItem(SPLIT_RATIO_KEY, String(ratio));
    }
  };

  const startResize = (event: MouseEvent) => {
    event.preventDefault();
    const container = resizerEl.parentElement;
    if (!container) return;
    isResizing = true;
    document.body.classList.add('resizing-panes');

    const handleMove = (moveEvent: MouseEvent) => {
      const rect = container.getBoundingClientRect();
      if (rect.width <= 0) return;
      const next = (moveEvent.clientX - rect.left) / rect.width;
      ratio = Math.min(MAX_RATIO, Math.max(MIN_RATIO, next));
    };

    const handleUp = () => {
      isResizing = false;
      document.body.classList.remove('resizing-panes');
      saveRatio();
      window.removeEventListener('mousemove', handleMove);
      window.removeEventListener('mouseup', handleUp);
    };

    window.addEventListener('mousemove', handleMove);
    window.addEventListener('mouseup', handleUp);
  };

  onMount(() => {
    ratio = loadRatio();
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="pane-resizer" class:active={isResizing} bind:this={resizerEl} on:mousedown={startResize}></div>

<style>
  .pane-resizer {
    flex-shrink: 0;
    width: 5px;
    margin-left: -2px;
    margin-right: -2px;
    z-index: 10;
    cursor: col-resize;
    background: transparent;
    border-right: 1px solid var(--border);
  }

  .pane-resizer:hover {
    border-right: 1px solid var(--muted);
  }

  .pane-resizer.active {
    border-right: 2px solid var(--accent);
  }
</style>

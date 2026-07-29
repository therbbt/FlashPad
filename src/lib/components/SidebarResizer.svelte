<script lang="ts">
  import { onMount } from 'svelte';

  const SIDEBAR_WIDTH_KEY = 'flashpad.sidebarWidth';
  const SIDEBAR_MIN_WIDTH = 80;
  const SIDEBAR_MAX_WIDTH = 480;
  const DEFAULT_SIDEBAR_WIDTH = 260;

  export let width = DEFAULT_SIDEBAR_WIDTH;

  let isResizing = false;

  const loadWidth = (): number => {
    if (typeof window === 'undefined') return DEFAULT_SIDEBAR_WIDTH;
    const raw = Number(window.localStorage.getItem(SIDEBAR_WIDTH_KEY));
    if (!raw || Number.isNaN(raw)) return DEFAULT_SIDEBAR_WIDTH;
    return Math.min(SIDEBAR_MAX_WIDTH, Math.max(SIDEBAR_MIN_WIDTH, raw));
  };

  const saveWidth = () => {
    if (typeof window !== 'undefined') {
      window.localStorage.setItem(SIDEBAR_WIDTH_KEY, String(width));
    }
  };

  const startResize = (event: MouseEvent) => {
    event.preventDefault();
    isResizing = true;
    document.body.classList.add('resizing-sidebar');

    const handleMove = (moveEvent: MouseEvent) => {
      width = Math.min(SIDEBAR_MAX_WIDTH, Math.max(SIDEBAR_MIN_WIDTH, moveEvent.clientX));
    };

    const handleUp = () => {
      isResizing = false;
      document.body.classList.remove('resizing-sidebar');
      saveWidth();
      window.removeEventListener('mousemove', handleMove);
      window.removeEventListener('mouseup', handleUp);
    };

    window.addEventListener('mousemove', handleMove);
    window.addEventListener('mouseup', handleUp);
  };

  onMount(() => {
    width = loadWidth();
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="sidebar-resizer" class:active={isResizing} on:mousedown={startResize}></div>

<style>
  .sidebar-resizer {
    flex-shrink: 0;
    width: 5px;
    margin-left: -2px;
    margin-right: -2px;
    z-index: 10;
    cursor: col-resize;
    background: transparent;
    border-right: 1px solid var(--border);
  }

  .sidebar-resizer:hover {
    border-right: 1px solid var(--muted);
  }

  .sidebar-resizer.active {
    border-right: 2px solid var(--accent);
  }
</style>

<script lang="ts">
  import { onMount } from 'svelte';

  export interface ContextMenuItem {
    label: string;
    action?: () => void;
    danger?: boolean;
    separator?: boolean;
    disabled?: boolean;
    submenu?: ContextMenuItem[];
  }

  export let x: number;
  export let y: number;
  export let items: ContextMenuItem[];
  export let onClose: () => void;

  let menuEl: HTMLDivElement;
  let openSubmenuIndex: number | null = null;
  let submenuPos = { x: 0, y: 0 };

  let adjustedX = x;
  let adjustedY = y;

  const clampToViewport = () => {
    if (!menuEl) return;
    const rect = menuEl.getBoundingClientRect();
    const margin = 6;
    adjustedX = Math.min(x, window.innerWidth - rect.width - margin);
    adjustedY = Math.min(y, window.innerHeight - rect.height - margin);
    adjustedX = Math.max(adjustedX, margin);
    adjustedY = Math.max(adjustedY, margin);
  };

  const openSubmenu = (index: number, event: MouseEvent) => {
    const target = event.currentTarget as HTMLElement;
    const rect = target.getBoundingClientRect();
    submenuPos = { x: rect.right, y: rect.top };
    openSubmenuIndex = index;
  };

  const runItem = (item: ContextMenuItem) => {
    if (item.disabled || item.separator || item.submenu) return;
    item.action?.();
    onClose();
  };

  const handleOutside = (event: MouseEvent) => {
    if (menuEl && !menuEl.contains(event.target as Node)) {
      onClose();
    }
  };

  const handleKeydown = (event: KeyboardEvent) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      onClose();
    }
  };

  onMount(() => {
    clampToViewport();
    window.addEventListener('mousedown', handleOutside, true);
    window.addEventListener('keydown', handleKeydown, true);
    window.addEventListener('blur', onClose);
    return () => {
      window.removeEventListener('mousedown', handleOutside, true);
      window.removeEventListener('keydown', handleKeydown, true);
      window.removeEventListener('blur', onClose);
    };
  });
</script>

<div class="menu" bind:this={menuEl} style="left: {adjustedX}px; top: {adjustedY}px;">
  {#each items as item, index (item.label + index)}
    {#if item.separator}
      <div class="separator"></div>
    {:else}
      <button
        class="item"
        class:danger={item.danger}
        class:disabled={item.disabled}
        on:click={() => runItem(item)}
        on:mouseenter={(e) => (item.submenu ? openSubmenu(index, e) : (openSubmenuIndex = null))}
        disabled={item.disabled}
      >
        <span>{item.label}</span>
        {#if item.submenu}
          <span class="chevron">›</span>
        {/if}
      </button>
      {#if item.submenu && openSubmenuIndex === index}
        <div class="submenu" style="left: {submenuPos.x}px; top: {submenuPos.y}px;">
          {#each item.submenu as subitem, subindex (subitem.label + subindex)}
            {#if subitem.separator}
              <div class="separator"></div>
            {:else}
              <button class="item" class:disabled={subitem.disabled} on:click={() => runItem(subitem)} disabled={subitem.disabled}>
                <span>{subitem.label}</span>
              </button>
            {/if}
          {/each}
        </div>
      {/if}
    {/if}
  {/each}
</div>

<style>
  .menu,
  .submenu {
    position: fixed;
    z-index: 1000;
    min-width: 170px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
    padding: 0.25rem;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    border: 0;
    background: transparent;
    color: inherit;
    text-align: left;
    padding: 0.35rem 0.5rem;
    font-size: 0.8rem;
    border-radius: 0.3rem;
    cursor: pointer;
  }

  .item:hover:not(.disabled) {
    background: var(--panel-2);
  }

  .item.danger {
    color: #ef4444;
  }

  .item.disabled {
    opacity: 0.4;
    cursor: default;
  }

  .chevron {
    color: var(--muted);
    font-size: 0.75rem;
  }

  .separator {
    height: 1px;
    background: var(--border);
    margin: 0.25rem 0.2rem;
  }
</style>

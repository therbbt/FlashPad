<script lang="ts">
  import { onMount } from 'svelte';

  export let onClose: () => void;

  let panelEl: HTMLDivElement;

  const rules: [string, string][] = [
    ['# Heading 1', 'Largest heading'],
    ['## Heading 2', 'Medium heading'],
    ['### Heading 3', 'Small heading'],
    ['**bold**', 'Bold text'],
    ['*italic*', 'Italic text'],
    ['~~strike~~', 'Strikethrough text'],
    ['`code`', 'Inline code'],
    ['``` at line start', 'Code block'],
    ['> text', 'Blockquote'],
    ['- text', 'Bullet list'],
    ['1. text', 'Numbered list'],
    ['---', 'Horizontal divider'],
  ];

  const handleKeydown = (event: KeyboardEvent) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      onClose();
    }
  };

  const handleOutsideClick = (event: MouseEvent) => {
    if (panelEl && !panelEl.contains(event.target as Node)) {
      onClose();
    }
  };

  onMount(() => {
    window.addEventListener('mousedown', handleOutsideClick, true);
    return () => {
      window.removeEventListener('mousedown', handleOutsideClick, true);
    };
  });
</script>

<svelte:window on:keydown={handleKeydown} />

<div class="overlay">
  <div class="panel" bind:this={panelEl} role="dialog" aria-modal="true" aria-label="Markdown guide">
    <header>
      <h2>Markdown guide</h2>
      <button class="close" on:click={onClose} aria-label="Close">
        <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
          <path d="M2 2l12 12M14 2L2 14" />
        </svg>
      </button>
    </header>
    <p class="hint">Type these at the start of a line (or around text) and they format as you type.</p>
    <table>
      <tbody>
        {#each rules as [syntax, description] (syntax)}
          <tr>
            <td><code>{syntax}</code></td>
            <td>{description}</td>
          </tr>
        {/each}
      </tbody>
    </table>
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
    z-index: 1100;
  }

  .panel {
    width: min(360px, 90vw);
    max-height: 80vh;
    overflow: auto;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 0.6rem;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.4);
    padding: 0.75rem;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.5rem;
  }

  h2 {
    margin: 0;
    font-size: 0.9rem;
  }

  .close {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 1.5rem;
    height: 1.5rem;
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    background: var(--panel-2);
    color: var(--muted);
    padding: 0;
  }

  .close:hover {
    color: var(--text);
  }

  .hint {
    margin: 0 0 0.5rem;
    font-size: 0.72rem;
    color: var(--muted);
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.78rem;
  }

  td {
    padding: 0.4rem 0.3rem;
    border-top: 1px solid var(--border);
    color: var(--text);
    vertical-align: top;
  }

  tr:first-child td {
    border-top: none;
  }

  td:first-child {
    white-space: nowrap;
    width: 1%;
    padding-right: 0.75rem;
  }

  td:last-child {
    color: var(--muted);
  }

  code {
    font-family: inherit;
    font-size: 0.72rem;
    background: var(--panel-2);
    border: 1px solid var(--border);
    border-radius: 0.3rem;
    padding: 0.15rem 0.4rem;
  }
</style>

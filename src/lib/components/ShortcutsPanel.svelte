<script lang="ts">
  export let hotkey: string;
  export let onClose: () => void;

  let panelEl: HTMLDivElement;

  $: shortcutGroups = [
    {
      label: 'General',
      items: [
        [hotkey, 'Open / restore FlashPad from anywhere'],
        ['Esc', 'Hide the window (still running in the tray)'],
      ] as [string, string][],
    },
    {
      label: 'Notes',
      items: [
        ['Alt+N', 'Create a new note'],
        ['Alt+L', 'Lock / unlock the current note'],
        ['Alt+D', 'Delete the current note (and its subnotes)'],
        ['Alt+M', 'Toggle Markdown view'],
      ] as [string, string][],
    },
    {
      label: 'Insert in note',
      items: [
        ['Alt+1', 'Insert a divider line'],
        ['Alt+2', 'Insert a timestamp'],
        ['Alt+3', 'Insert a dateline'],
      ] as [string, string][],
    },
    {
      label: 'Database',
      items: [
        ['Alt+B', 'Switch to the next database'],
      ] as [string, string][],
    },
    {
      label: 'Navigation',
      items: [
        ['Enter', "Open the focused note, toggling its subnotes if it has any"],
        ['↑ / ↓', 'Move through the tree or search results'],
        ['← / →', "Collapse / expand the focused note's subnotes"],
      ] as [string, string][],
    },
    {
      label: 'Mouse & renaming',
      items: [
        ['Right-click a note', 'New subnote, rename, duplicate, move, lock, delete'],
        ['Right-click the text', 'Copy, cut, or paste the note; lock / unlock'],
        ['Enter / Esc', 'While renaming: confirm / cancel'],
      ] as [string, string][],
    },
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

</script>

<svelte:window on:keydown={handleKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="overlay" on:mousedown={handleOutsideClick}>
  <div class="panel" bind:this={panelEl} role="dialog" aria-modal="true" aria-label="Keyboard shortcuts">
    <div class="panel-inner">
      <header>
        <h2>Keyboard shortcuts</h2>
        <button class="close" on:click={onClose} aria-label="Close">
          <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
            <path d="M2 2l12 12M14 2L2 14" />
          </svg>
        </button>
      </header>
      <table>
        {#each shortcutGroups as group (group.label)}
          <tbody>
            <tr class="group-row">
              <th colspan="2">{group.label}</th>
            </tr>
            {#each group.items as [keys, description] (keys)}
              <tr>
                <td><kbd>{keys}</kbd></td>
                <td>{description}</td>
              </tr>
            {/each}
          </tbody>
        {/each}
      </table>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: var(--window-shadow-margin, 0);
    background: rgba(0, 0, 0, 0.35);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1100;
  }

  .panel {
    width: min(460px, 92vw);
    max-height: 85vh;
    overflow: auto;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 0.6rem;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.4);
    /* Puts the scrollbar on the left edge of the panel instead of the
       right - panel-inner resets back to ltr so text/layout read normally. */
    direction: rtl;
  }

  .panel-inner {
    direction: ltr;
    padding: 0.85rem 1rem;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.5rem;
  }

  h2 {
    margin: 0;
    font-size: 1rem;
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

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.85rem;
  }

  td {
    padding: 0.5rem 0.4rem;
    border-top: 1px solid var(--border);
    color: var(--text);
    vertical-align: top;
  }

  .group-row th {
    text-align: left;
    padding: 0.7rem 0.4rem 0.3rem;
    font-size: 0.72rem;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--muted);
  }

  tbody:first-of-type .group-row th {
    padding-top: 0;
  }

  tbody:not(:first-of-type) .group-row th {
    border-top: 1px solid var(--border);
  }

  td:first-child {
    white-space: nowrap;
    width: 1%;
    padding-right: 0.75rem;
  }

  td:last-child {
    color: var(--muted);
  }

  kbd {
    font-family: inherit;
    font-size: 0.78rem;
    background: var(--panel-2);
    border: 1px solid var(--border);
    border-radius: 0.3rem;
    padding: 0.2rem 0.45rem;
  }
</style>

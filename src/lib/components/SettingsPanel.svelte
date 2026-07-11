<script lang="ts">
  import { onMount } from 'svelte';
  import { AutostartService } from '../services/autostartService';
  import { HotkeyService } from '../services/hotkeyService';

  export let hotkey: string;
  export let onHotkeyChange: (hotkey: string) => void;
  export let onClose: () => void;

  const autostartService = new AutostartService();
  const hotkeyService = new HotkeyService();

  let panelEl: HTMLDivElement;
  let autostart = false;
  let loading = true;
  let error = '';

  let ctrlMod = false;
  let altMod = false;
  let shiftMod = false;
  let superMod = false;
  let keyInput = '';
  let hotkeySaving = false;
  let hotkeySaved = false;
  let hotkeyError = '';

  $: canSaveHotkey = (ctrlMod || altMod || shiftMod || superMod) && keyInput.trim().length > 0;

  const parseHotkey = (value: string) => {
    ctrlMod = false;
    altMod = false;
    shiftMod = false;
    superMod = false;
    keyInput = '';
    for (const rawToken of value.split('+')) {
      const token = rawToken.trim().toUpperCase();
      if (!token) continue;
      if (token === 'CTRL' || token === 'CONTROL') ctrlMod = true;
      else if (token === 'ALT' || token === 'OPTION') altMod = true;
      else if (token === 'SHIFT') shiftMod = true;
      else if (token === 'SUPER' || token === 'CMD' || token === 'COMMAND') superMod = true;
      else keyInput = token;
    }
  };

  const buildHotkey = (): string => {
    const mods: string[] = [];
    if (ctrlMod) mods.push('Ctrl');
    if (altMod) mods.push('Alt');
    if (shiftMod) mods.push('Shift');
    if (superMod) mods.push('Super');
    const key = keyInput.trim().toUpperCase();
    return mods.length && key ? [...mods, key].join('+') : '';
  };

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

  const toggleAutostart = async () => {
    const next = !autostart;
    error = '';
    try {
      await autostartService.setEnabled(next);
      autostart = next;
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to update autostart';
    }
  };

  const saveHotkey = async () => {
    const built = buildHotkey();
    if (!built) return;
    hotkeyError = '';
    hotkeySaved = false;
    hotkeySaving = true;
    try {
      await hotkeyService.set(built);
      onHotkeyChange(built);
      hotkeySaved = true;
    } catch (err) {
      hotkeyError = err instanceof Error ? err.message : 'Failed to update hotkey';
    } finally {
      hotkeySaving = false;
    }
  };

  onMount(() => {
    parseHotkey(hotkey);
    window.addEventListener('mousedown', handleOutsideClick, true);
    (async () => {
      try {
        autostart = await autostartService.isEnabled();
      } catch {
        // leave at default (off)
      } finally {
        loading = false;
      }
    })();
    return () => {
      window.removeEventListener('mousedown', handleOutsideClick, true);
    };
  });
</script>

<svelte:window on:keydown={handleKeydown} />

<div class="overlay">
  <div class="panel" bind:this={panelEl} role="dialog" aria-modal="true" aria-label="Settings">
    <header>
      <h2>Settings</h2>
      <button class="close" on:click={onClose} aria-label="Close">
        <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
          <path d="M2 2l12 12M14 2L2 14" />
        </svg>
      </button>
    </header>

    <label class="row">
      <span>Start FlashPad when you log in</span>
      <input type="checkbox" checked={autostart} disabled={loading} on:change={toggleAutostart} />
    </label>
    {#if error}
      <p class="error">{error}</p>
    {/if}

    <div class="section">
      <span class="section-title">Toggle hotkey</span>
      <div class="hotkey-row">
        <label class="mod"><input type="checkbox" bind:checked={ctrlMod} /> Ctrl</label>
        <label class="mod"><input type="checkbox" bind:checked={altMod} /> Alt</label>
        <label class="mod"><input type="checkbox" bind:checked={shiftMod} /> Shift</label>
        <label class="mod"><input type="checkbox" bind:checked={superMod} /> Win</label>
        <input
          class="key-input"
          type="text"
          maxlength="1"
          placeholder="S"
          bind:value={keyInput}
          on:input={() => {
            hotkeySaved = false;
          }}
        />
        <button
          class="save-btn"
          disabled={!canSaveHotkey || hotkeySaving}
          on:click={saveHotkey}
        >
          {hotkeySaving ? 'Saving…' : 'Save'}
        </button>
      </div>
      {#if hotkeySaved}
        <p class="saved-hint">Saved</p>
      {/if}
      {#if hotkeyError}
        <p class="error">{hotkeyError}</p>
      {/if}
    </div>
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

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    font-size: 0.8rem;
    color: var(--text);
    padding: 0.4rem 0.2rem;
    cursor: pointer;
  }

  .row input {
    flex-shrink: 0;
  }

  .section {
    margin-top: 0.6rem;
    padding-top: 0.6rem;
    border-top: 1px solid var(--border);
  }

  .section-title {
    display: block;
    font-size: 0.8rem;
    color: var(--text);
    margin-bottom: 0.5rem;
  }

  .hotkey-row {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.6rem;
  }

  .mod {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    font-size: 0.75rem;
    color: var(--muted);
    cursor: pointer;
    white-space: nowrap;
  }

  .key-input {
    width: 2.2rem;
    text-align: center;
    text-transform: uppercase;
    border: 1px solid var(--border);
    border-radius: 0.35rem;
    background: var(--panel-2);
    color: inherit;
    font-size: 0.8rem;
    padding: 0.3rem 0.2rem;
  }

  .save-btn {
    border: 1px solid var(--border);
    border-radius: 0.35rem;
    background: var(--panel-2);
    color: inherit;
    font-size: 0.78rem;
    padding: 0.3rem 0.6rem;
    cursor: pointer;
    margin-left: auto;
  }

  .save-btn:hover:not(:disabled) {
    background: var(--accent-soft, var(--panel-2));
  }

  .save-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .saved-hint {
    margin: 0.4rem 0 0;
    font-size: 0.75rem;
    color: var(--muted);
  }

  .error {
    margin: 0.4rem 0 0;
    font-size: 0.75rem;
    color: #ef4444;
  }
</style>

<script lang="ts">
  import { onMount } from 'svelte';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { AutostartService } from '../services/autostartService';
  import { HotkeyService } from '../services/hotkeyService';
  import { DatabaseService, type AppState } from '../services/databaseService';
  import { BackupService } from '../services/backupService';

  export let hotkey: string;
  export let onHotkeyChange: (hotkey: string) => void;
  export let onClose: () => void;
  export let onOpenDatabaseManager: () => void;
  export let onRequestConfirm: (message: string) => Promise<boolean>;
  // Called after an import replaces the active database's contents, so
  // App.svelte can reload notes and reset note-scoped UI state. NOT called
  // after a location change - the data itself is unchanged, only where it
  // lives on disk, so there's nothing for the notes view to refresh.
  export let onImported: () => Promise<void>;
  // Called after a manual "Reload database" - unlike onImported this can
  // report the database as unreachable (reload_database resolves even when
  // reactivation fails), so App.svelte needs the AppState itself to decide
  // whether to show the notes view or the startup-error view.
  export let onReloaded: (state: AppState) => Promise<void>;

  const autostartService = new AutostartService();
  const hotkeyService = new HotkeyService();
  const databaseService = new DatabaseService();
  const backupService = new BackupService();

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

  let appState: AppState | null = null;
  let dbBusy = false;
  let dbMessage = '';
  let dbError = '';
  let retentionInput = 7;
  let retentionSaving = false;

  $: canSaveHotkey = (ctrlMod || altMod || shiftMod || superMod) && keyInput.trim().length > 0;
  $: activeDatabase = appState?.databases.find((d) => d.id === appState?.activeDatabaseId) ?? null;

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

  const loadDatabaseSection = async () => {
    try {
      appState = await databaseService.getAppState();
      if (appState) retentionInput = appState.backup.retentionCount;
    } catch (err) {
      dbError = err instanceof Error ? err.message : 'Failed to load database settings';
    }
  };

  const saveRetentionCount = async () => {
    retentionSaving = true;
    dbError = '';
    try {
      await backupService.setRetentionCount(retentionInput);
      await loadDatabaseSection();
    } catch (err) {
      dbError = err instanceof Error ? err.message : 'Failed to save retention count';
    } finally {
      retentionSaving = false;
    }
  };

  const changeLocation = async () => {
    const picked = await save({
      defaultPath: activeDatabase?.name ? `${activeDatabase.name}.sqlite3` : 'flashpad.sqlite3',
      filters: [{ name: 'FlashPad database', extensions: ['sqlite3', 'db'] }],
    });
    if (!picked) return;
    dbBusy = true;
    dbError = '';
    dbMessage = '';
    try {
      await databaseService.setDatabasePath(picked);
      await loadDatabaseSection();
      dbMessage = 'Database location updated. The previous file was left in place.';
    } catch (err) {
      dbError = err instanceof Error ? err.message : 'Failed to change database location';
    } finally {
      dbBusy = false;
    }
  };

  const exportDatabase = async () => {
    const today = new Date().toISOString().slice(0, 10);
    const picked = await save({
      defaultPath: `flashpad-export-${today}.db`,
      filters: [{ name: 'FlashPad database', extensions: ['db', 'sqlite3'] }],
    });
    if (!picked) return;
    dbBusy = true;
    dbError = '';
    dbMessage = '';
    try {
      await backupService.exportTo(picked);
      dbMessage = 'Exported successfully.';
    } catch (err) {
      dbError = err instanceof Error ? err.message : 'Failed to export database';
    } finally {
      dbBusy = false;
    }
  };

  const importDatabase = async () => {
    const picked = await open({
      multiple: false,
      filters: [{ name: 'FlashPad database', extensions: ['db', 'sqlite3'] }],
    });
    if (typeof picked !== 'string') return;

    const ok = await onRequestConfirm(
      'Importing will replace ALL notes in the currently active database with the contents of the selected file. A safety backup of your current data will be created automatically first. Continue?',
    );
    if (!ok) return;

    dbBusy = true;
    dbError = '';
    dbMessage = '';
    try {
      await backupService.importFrom(picked);
      await onImported();
      dbMessage = 'Import complete.';
    } catch (err) {
      dbError = err instanceof Error ? err.message : 'Failed to import database';
    } finally {
      dbBusy = false;
    }
  };

  const backupNow = async () => {
    dbBusy = true;
    dbError = '';
    dbMessage = '';
    try {
      await backupService.createNow();
      dbMessage = 'Backup created.';
    } catch (err) {
      dbError = err instanceof Error ? err.message : 'Failed to create backup';
    } finally {
      dbBusy = false;
    }
  };

  const reloadDatabase = async () => {
    dbBusy = true;
    dbError = '';
    dbMessage = '';
    try {
      const state = await databaseService.reloadDatabase();
      appState = state;
      await onReloaded(state);
      if (state.ready) dbMessage = 'Database reloaded.';
    } catch (err) {
      dbError = err instanceof Error ? err.message : 'Failed to reload database';
    } finally {
      dbBusy = false;
    }
  };

  onMount(() => {
    parseHotkey(hotkey);
    window.addEventListener('mousedown', handleOutsideClick, true);
    void loadDatabaseSection();
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

    <div class="section">
      <span class="section-title">Database</span>

      {#if activeDatabase}
        <p class="db-current">
          <strong>{activeDatabase.name}</strong>
          <span class="db-path" title={activeDatabase.path}>{activeDatabase.path}</span>
        </p>
      {/if}

      {#if appState?.syncWarning}
        <p class="sync-warning">{appState.syncWarning}</p>
      {/if}

      <p class="hint">Backups are stored locally on this device only, even if the database itself lives in a synced folder.</p>

      <div class="db-buttons">
        <button class="save-btn" disabled={dbBusy} on:click={changeLocation}>Change location…</button>
        <button class="save-btn" disabled={dbBusy} on:click={exportDatabase}>Export…</button>
        <button class="save-btn" disabled={dbBusy} on:click={importDatabase}>Import…</button>
        <button class="save-btn" disabled={dbBusy} on:click={backupNow}>Back up now</button>
        <button
          class="save-btn"
          disabled={dbBusy}
          on:click={reloadDatabase}
          title="Re-reads the database file from disk - use this after a sync client (OneDrive, Dropbox) pulls down changes made on another device"
        >
          Reload database
        </button>
      </div>

      <div class="retention-row">
        <span>Keep last</span>
        <input class="retention-input" type="number" min="1" bind:value={retentionInput} />
        <span>backups</span>
        <button class="save-btn" disabled={retentionSaving} on:click={saveRetentionCount}>
          {retentionSaving ? 'Saving…' : 'Save'}
        </button>
      </div>

      <button class="manage-link" on:click={onOpenDatabaseManager}>Manage databases…</button>

      {#if dbMessage}
        <p class="saved-hint">{dbMessage}</p>
      {/if}
      {#if dbError}
        <p class="error">{dbError}</p>
      {/if}
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

  .db-current {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    margin: 0 0 0.5rem;
    font-size: 0.8rem;
    color: var(--text);
  }

  .db-path {
    font-size: 0.7rem;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .sync-warning {
    margin: 0 0 0.5rem;
    font-size: 0.72rem;
    line-height: 1.4;
    color: var(--muted);
    background: var(--panel-2);
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    padding: 0.4rem 0.5rem;
  }

  .hint {
    margin: 0 0 0.5rem;
    font-size: 0.72rem;
    color: var(--muted);
  }

  .db-buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    margin-bottom: 0.6rem;
  }

  .db-buttons .save-btn {
    margin-left: 0;
  }

  .retention-row {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.78rem;
    color: var(--muted);
    margin-bottom: 0.6rem;
  }

  .retention-row .save-btn {
    margin-left: auto;
  }

  .retention-input {
    width: 3rem;
    text-align: center;
    border: 1px solid var(--border);
    border-radius: 0.35rem;
    background: var(--panel-2);
    color: inherit;
    font-size: 0.78rem;
    padding: 0.25rem 0.2rem;
  }

  .manage-link {
    display: block;
    background: none;
    border: none;
    color: var(--muted);
    font-size: 0.75rem;
    text-decoration: underline;
    cursor: pointer;
    padding: 0;
  }

  .manage-link:hover {
    color: var(--text);
  }
</style>

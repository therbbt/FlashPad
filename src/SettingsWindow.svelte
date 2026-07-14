<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { emit, listen } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api/core';
  import { AutostartService } from './lib/services/autostartService';
  import { HotkeyService } from './lib/services/hotkeyService';
  import { DatabaseService, type AppState } from './lib/services/databaseService';
  import { BackupService } from './lib/services/backupService';
  import { SettingsService } from './lib/services/settingsService';
  import TitleBar from './lib/components/TitleBar.svelte';
  import ResizeHandles from './lib/components/ResizeHandles.svelte';
  import ConfirmDialog from './lib/components/ConfirmDialog.svelte';

  const autostartService = new AutostartService();
  const hotkeyService = new HotkeyService();
  const settingsService = new SettingsService();
  const databaseService = new DatabaseService();
  const backupService = new BackupService();

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

  let confirmState: { message: string; resolve: (value: boolean) => void } | null = null;

  $: canSaveHotkey = (ctrlMod || altMod || shiftMod || superMod) && keyInput.trim().length > 0;
  $: activeDatabase = appState?.databases.find((d) => d.id === appState?.activeDatabaseId) ?? null;

  const requestConfirm = (message: string): Promise<boolean> => {
    return new Promise((resolve) => {
      confirmState = { message, resolve };
    });
  };

  const minimizeSelf = async () => {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    await getCurrentWindow().minimize();
  };

  const closeSelf = async () => {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    await getCurrentWindow().close();
  };

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
      await emit('hotkey-changed', built);
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

    const ok = await requestConfirm(
      'Importing will replace ALL notes in the currently active database with the contents of the selected file. A safety backup of your current data will be created automatically first. Continue?',
    );
    if (!ok) return;

    dbBusy = true;
    dbError = '';
    dbMessage = '';
    try {
      await backupService.importFrom(picked);
      await loadDatabaseSection();
      if (appState) await emit('database-changed', appState);
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
      await emit('database-changed', state);
      if (state.ready) dbMessage = 'Database reloaded.';
    } catch (err) {
      dbError = err instanceof Error ? err.message : 'Failed to reload database';
    } finally {
      dbBusy = false;
    }
  };

  const openDatabaseManager = () => {
    void invoke('open_database_window').catch((err) => console.error('open_database_window failed', err));
  };

  let unlistenTheme: (() => void) | undefined;

  onMount(() => {
    // Each window is a separate document, so the theme this window renders
    // with has to be loaded and applied here too - it isn't inherited from
    // the main window. The window is created invisible (see
    // open_settings_window in Rust) specifically so nothing shows before
    // the theme is applied and painted - the double rAF waits for the
    // browser to have actually painted with that theme, rather than
    // revealing a flash of the wrong (default dark) background first.
    const reveal = () =>
      requestAnimationFrame(() =>
        requestAnimationFrame(() =>
          void invoke('show_utility_window', { label: 'settings' }).catch((err) =>
            console.error('show_utility_window failed', err),
          ),
        ),
      );
    settingsService
      .load()
      .then((settings) => {
        document.documentElement.dataset.theme = settings.theme;
      })
      .catch(() => {
        // leave at default theme
      })
      .finally(reveal);
    void listen<'dark' | 'light'>('theme-changed', (event) => {
      document.documentElement.dataset.theme = event.payload;
    }).then((unlisten) => {
      unlistenTheme = unlisten;
    });
    (async () => {
      try {
        parseHotkey(await hotkeyService.get());
      } catch {
        parseHotkey('');
      }
    })();
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
  });

  onDestroy(() => unlistenTheme?.());
</script>

<svelte:head>
  <title>FlashPad Settings</title>
</svelte:head>

<div class="shell">
  <ResizeHandles />
  <TitleBar title="Settings" onMinimize={minimizeSelf} onClose={closeSelf} />

  <div class="content">
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

      <button class="manage-link" on:click={openDatabaseManager}>Manage databases…</button>

      {#if dbMessage}
        <p class="saved-hint">{dbMessage}</p>
      {/if}
      {#if dbError}
        <p class="error">{dbError}</p>
      {/if}
    </div>
  </div>
</div>

{#if confirmState}
  <ConfirmDialog
    message={confirmState.message}
    onConfirm={() => {
      confirmState?.resolve(true);
      confirmState = null;
    }}
    onCancel={() => {
      confirmState?.resolve(false);
      confirmState = null;
    }}
  />
{/if}

<style>
  .shell {
    position: fixed;
    inset: var(--window-shadow-margin);
    display: flex;
    flex-direction: column;
    background: var(--bg);
    color: var(--text);
    border-radius: 0.6rem;
    overflow: hidden;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.4);
  }

  .content {
    flex: 1;
    overflow: auto;
    padding: 0.75rem;
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

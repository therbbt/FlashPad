<script lang="ts">
  import { onMount } from 'svelte';
  import { getVersion } from '@tauri-apps/api/app';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { AutostartService } from '../services/autostartService';
  import { HotkeyService } from '../services/hotkeyService';
  import { DatabaseService, type AppState } from '../services/databaseService';
  import { BackupService } from '../services/backupService';
  import { palettesForMode, type Palette } from '../theme/palettes';
  import DatabaseManagerSection from './DatabaseManagerSection.svelte';

  export let hotkey: string;
  export let onHotkeyChange: (hotkey: string) => void;
  export let lightPaletteId: string;
  export let darkPaletteId: string;
  export let onLightPaletteChange: (id: string) => void;
  export let onDarkPaletteChange: (id: string) => void;
  export let onClose: () => void;
  export let onSwitchDatabase: (id: number) => Promise<void>;
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
  let tab: 'general' | 'database' = 'general';
  let autostart = false;
  let loading = true;
  let error = '';
  let appVersion = '';

  // <select>/<option> render as native OS popups on Linux (WebKitGTK), which
  // ignore page CSS entirely - a custom dropdown is the only way to get
  // themed colors here, not just a styling workaround.
  const lightPalettes = palettesForMode('light');
  const darkPalettes = palettesForMode('dark');
  let openPaletteDropdown: 'light' | 'dark' | null = null;

  $: currentLightPalette = lightPalettes.find((p) => p.id === lightPaletteId) ?? lightPalettes[0];
  $: currentDarkPalette = darkPalettes.find((p) => p.id === darkPaletteId) ?? darkPalettes[0];

  const togglePaletteDropdown = (which: 'light' | 'dark') => {
    openPaletteDropdown = openPaletteDropdown === which ? null : which;
  };

  const choosePalette = (which: 'light' | 'dark', palette: Palette) => {
    if (which === 'light') onLightPaletteChange(palette.id);
    else onDarkPaletteChange(palette.id);
    openPaletteDropdown = null;
  };

  const handleDropdownOutsideClick = (event: MouseEvent) => {
    const target = event.target as HTMLElement;
    if (!target.closest('.dropdown')) {
      openPaletteDropdown = null;
    }
  };

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
    void loadDatabaseSection();
    void getVersion().then((v) => (appVersion = v));
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
</script>

<svelte:window on:keydown={handleKeydown} on:mousedown={handleDropdownOutsideClick} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="overlay" on:mousedown={handleOutsideClick}>
  <div class="panel" bind:this={panelEl} role="dialog" aria-modal="true" aria-label="Settings">
    <header>
      <h2>Settings</h2>
      <button class="close" on:click={onClose} aria-label="Close">
        <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
          <path d="M2 2l12 12M14 2L2 14" />
        </svg>
      </button>
    </header>

    <nav class="tabs">
      <button class="tab" class:active={tab === 'general'} on:click={() => (tab = 'general')}>
        <svg width="13" height="13" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="8" cy="8" r="2.2" />
          <path d="M8 2v1.6M8 12.4V14M14 8h-1.6M3.6 8H2M12.13 3.87l-1.13 1.13M4.99 11.01l-1.13 1.13M12.13 12.13l-1.13-1.13M4.99 4.99 3.87 3.87" />
        </svg>
        General
      </button>
      <button class="tab" class:active={tab === 'database'} on:click={() => (tab = 'database')}>
        <svg width="13" height="13" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round">
          <ellipse cx="8" cy="3.5" rx="5.5" ry="2" />
          <path d="M2.5 3.5v9c0 1.1 2.46 2 5.5 2s5.5-.9 5.5-2v-9" />
          <path d="M2.5 8c0 1.1 2.46 2 5.5 2s5.5-.9 5.5-2" />
        </svg>
        Database
      </button>
    </nav>

    <div class="content">
      {#if tab === 'general'}
        <div class="pane">
          <section class="card">
            <label class="row">
              <span>Start FlashPad when you log in</span>
              <input type="checkbox" checked={autostart} disabled={loading} on:change={toggleAutostart} />
            </label>
            {#if error}
              <p class="error">{error}</p>
            {/if}
          </section>

          <section class="card">
            <span class="section-title">Appearance</span>
            <div class="palette-row">
              <span>Light theme</span>
              <div class="dropdown">
                <button
                  class="select"
                  type="button"
                  on:click={() => togglePaletteDropdown('light')}
                  aria-haspopup="listbox"
                  aria-expanded={openPaletteDropdown === 'light'}
                >
                  {currentLightPalette.name}
                  <svg class="caret" width="9" height="9" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M2.5 3.5L5 6.5L7.5 3.5" />
                  </svg>
                </button>
                {#if openPaletteDropdown === 'light'}
                  <ul class="dropdown-menu" role="listbox">
                    {#each lightPalettes as palette (palette.id)}
                      <li>
                        <button
                          class="dropdown-item"
                          class:active={palette.id === lightPaletteId}
                          role="option"
                          aria-selected={palette.id === lightPaletteId}
                          on:click={() => choosePalette('light', palette)}
                        >
                          {palette.name}
                        </button>
                      </li>
                    {/each}
                  </ul>
                {/if}
              </div>
            </div>
            <div class="palette-row">
              <span>Dark theme</span>
              <div class="dropdown">
                <button
                  class="select"
                  type="button"
                  on:click={() => togglePaletteDropdown('dark')}
                  aria-haspopup="listbox"
                  aria-expanded={openPaletteDropdown === 'dark'}
                >
                  {currentDarkPalette.name}
                  <svg class="caret" width="9" height="9" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M2.5 3.5L5 6.5L7.5 3.5" />
                  </svg>
                </button>
                {#if openPaletteDropdown === 'dark'}
                  <ul class="dropdown-menu" role="listbox">
                    {#each darkPalettes as palette (palette.id)}
                      <li>
                        <button
                          class="dropdown-item"
                          class:active={palette.id === darkPaletteId}
                          role="option"
                          aria-selected={palette.id === darkPaletteId}
                          on:click={() => choosePalette('dark', palette)}
                        >
                          {palette.name}
                        </button>
                      </li>
                    {/each}
                  </ul>
                {/if}
              </div>
            </div>
            <p class="hint">The app switches between these automatically with the light/dark toggle.</p>
          </section>

          <section class="card">
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
              <button class="btn" disabled={!canSaveHotkey || hotkeySaving} on:click={saveHotkey}>
                {hotkeySaving ? 'Saving…' : 'Save'}
              </button>
            </div>
            {#if hotkeySaved}
              <p class="saved-hint">Saved</p>
            {/if}
            {#if hotkeyError}
              <p class="error">{hotkeyError}</p>
            {/if}
          </section>
        </div>
      {:else}
        <div class="pane">
          <section class="card">
            <span class="section-title">Active database</span>
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

            <div class="action-grid">
              <button class="btn" disabled={dbBusy} on:click={changeLocation}>Change location…</button>
              <button class="btn" disabled={dbBusy} on:click={exportDatabase}>Export…</button>
              <button class="btn" disabled={dbBusy} on:click={importDatabase}>Import…</button>
              <button class="btn" disabled={dbBusy} on:click={backupNow}>Back up now</button>
              <button
                class="btn"
                disabled={dbBusy}
                on:click={reloadDatabase}
                title="Re-reads the database file from disk - use this after a sync client (OneDrive, Dropbox) pulls down changes made on another device"
              >
                Reload database
              </button>
            </div>

            <div class="retention-row">
              <span>Keep last</span>
              <div class="stepper">
                <button
                  class="step-btn"
                  type="button"
                  disabled={retentionInput <= 1}
                  on:click={() => (retentionInput = Math.max(1, retentionInput - 1))}
                  aria-label="Decrease"
                >
                  −
                </button>
                <input class="retention-input" type="number" min="1" bind:value={retentionInput} />
                <button class="step-btn" type="button" on:click={() => (retentionInput += 1)} aria-label="Increase">
                  +
                </button>
              </div>
              <span>backups</span>
              <button class="btn" disabled={retentionSaving} on:click={saveRetentionCount}>
                {retentionSaving ? 'Saving…' : 'Save'}
              </button>
            </div>

            {#if dbMessage}
              <p class="saved-hint">{dbMessage}</p>
            {/if}
            {#if dbError}
              <p class="error">{dbError}</p>
            {/if}
          </section>

          <section class="card">
            <span class="section-title">All databases</span>
            <DatabaseManagerSection onSwitch={onSwitchDatabase} {onRequestConfirm} />
          </section>
        </div>
      {/if}
    </div>
    {#if appVersion}
      <footer class="version-footer">FlashPad v{appVersion}</footer>
    {/if}
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
    width: min(500px, 92vw);
    max-height: 82vh;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 0.6rem;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.4);
    overflow: hidden;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.75rem 0.75rem 0;
    flex-shrink: 0;
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

  .tabs {
    display: flex;
    gap: 0.25rem;
    padding: 0.75rem 0.75rem 0;
    flex-shrink: 0;
    border-bottom: 1px solid var(--border);
  }

  .tab {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    color: var(--muted);
    font-size: 0.8rem;
    padding: 0.45rem 0.3rem 0.6rem;
    margin-bottom: -1px;
    cursor: pointer;
  }

  .tab:hover {
    color: var(--text);
  }

  .tab.active {
    color: var(--text);
    border-bottom-color: var(--accent);
  }

  .content {
    flex: 1;
    overflow: auto;
    padding: 0.75rem;
  }

  .version-footer {
    flex-shrink: 0;
    text-align: center;
    font-size: 0.68rem;
    color: var(--muted);
    padding: 0.4rem 0;
    border-top: 1px solid var(--border);
  }

  .pane {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }

  .card {
    border: 1px solid var(--border);
    border-radius: 0.5rem;
    background: var(--panel);
    padding: 0.65rem 0.7rem;
  }

  .section-title {
    display: block;
    font-size: 0.8rem;
    font-weight: 600;
    color: var(--text);
    margin-bottom: 0.5rem;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    font-size: 0.8rem;
    color: var(--text);
    cursor: pointer;
  }

  .row input {
    flex-shrink: 0;
  }

  .palette-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    font-size: 0.8rem;
    color: var(--text);
    margin-bottom: 0.5rem;
  }

  .palette-row:last-of-type {
    margin-bottom: 0.3rem;
  }

  .dropdown {
    position: relative;
  }

  .select {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    border: 1px solid var(--border);
    border-radius: 0.35rem;
    background: var(--panel-2);
    color: var(--text);
    font-size: 0.78rem;
    padding: 0.3rem 0.5rem;
    cursor: pointer;
  }

  .select:hover {
    background: var(--accent-soft, var(--panel-2));
  }

  .select .caret {
    color: var(--muted);
    flex-shrink: 0;
  }

  .dropdown-menu {
    position: absolute;
    top: calc(100% + 0.25rem);
    right: 0;
    z-index: 10;
    min-width: 100%;
    list-style: none;
    margin: 0;
    padding: 0.25rem;
    background: var(--panel-2);
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .dropdown-item {
    display: block;
    width: 100%;
    border: 0;
    background: transparent;
    color: var(--text);
    text-align: left;
    white-space: nowrap;
    padding: 0.35rem 0.6rem;
    font-size: 0.78rem;
    border-radius: 0.3rem;
    cursor: pointer;
  }

  .dropdown-item:hover {
    background: var(--panel);
  }

  .dropdown-item.active {
    color: var(--accent);
    font-weight: 600;
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

  .btn {
    border: 1px solid var(--border);
    border-radius: 0.35rem;
    background: var(--panel-2);
    color: inherit;
    font-size: 0.78rem;
    padding: 0.35rem 0.6rem;
    cursor: pointer;
  }

  .btn:hover:not(:disabled) {
    background: var(--accent-soft, var(--panel-2));
  }

  .btn:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .saved-hint {
    margin: 0.5rem 0 0;
    font-size: 0.75rem;
    color: var(--muted);
  }

  .error {
    margin: 0.5rem 0 0;
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
    margin: 0 0 0.6rem;
    font-size: 0.72rem;
    color: var(--muted);
  }

  .action-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 0.4rem;
    margin-bottom: 0.6rem;
  }

  .retention-row {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.78rem;
    color: var(--muted);
  }

  .retention-row .btn {
    margin-left: auto;
  }

  .stepper {
    display: flex;
    align-items: stretch;
  }

  /* Native number-input spin arrows are, like <select>/<option>, rendered
     as unstylable OS widgets on Linux (WebKitGTK) - hidden here in favor of
     the explicit .step-btn buttons instead. */
  .retention-input {
    appearance: textfield;
    -webkit-appearance: none;
    width: 2.4rem;
    text-align: center;
    border: 1px solid var(--border);
    border-left: 0;
    border-right: 0;
    background: var(--panel-2);
    color: var(--text);
    font-size: 0.78rem;
    padding: 0.25rem 0.1rem;
  }

  .retention-input:focus {
    outline: none;
  }

  .retention-input::-webkit-outer-spin-button,
  .retention-input::-webkit-inner-spin-button {
    -webkit-appearance: none;
    margin: 0;
  }

  .step-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 1.4rem;
    border: 1px solid var(--border);
    background: var(--panel-2);
    color: var(--text);
    font-size: 0.85rem;
    line-height: 1;
    cursor: pointer;
  }

  .step-btn:first-child {
    border-radius: 0.35rem 0 0 0.35rem;
  }

  .step-btn:last-child {
    border-radius: 0 0.35rem 0.35rem 0;
  }

  .step-btn:hover:not(:disabled) {
    background: var(--accent-soft, var(--panel-2));
  }

  .step-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }
</style>

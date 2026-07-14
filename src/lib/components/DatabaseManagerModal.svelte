<script lang="ts">
  import { onMount } from 'svelte';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { DatabaseService, type DatabaseProfile } from '../services/databaseService';

  export let onClose: () => void;
  export let onSwitch: (id: number) => Promise<void>;
  export let onRequestConfirm: (message: string) => Promise<boolean>;

  const databaseService = new DatabaseService();

  let panelEl: HTMLDivElement;
  let databases: DatabaseProfile[] = [];
  let activeId: number | null = null;
  let loading = true;
  let error = '';
  let switching = false;

  let renamingId: number | null = null;
  let renameValue = '';

  let addMode: 'new' | 'existing' | null = null;
  let addName = '';
  let addPath = '';
  let addBusy = false;

  const refresh = async () => {
    const state = await databaseService.getAppState();
    databases = await databaseService.listDatabases();
    activeId = state?.activeDatabaseId ?? null;
  };

  onMount(async () => {
    try {
      await refresh();
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to load databases';
    } finally {
      loading = false;
    }
  });

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

  const startRename = (db: DatabaseProfile) => {
    renamingId = db.id;
    renameValue = db.name;
  };

  const commitRename = async (id: number) => {
    const name = renameValue.trim();
    renamingId = null;
    if (!name) return;
    try {
      await databaseService.renameDatabase(id, name);
      await refresh();
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to rename database';
    }
  };

  const removeDatabase = async (db: DatabaseProfile) => {
    const ok = await onRequestConfirm(
      `Remove "${db.name}" from the list? Its file at ${db.path} will NOT be deleted.`,
    );
    if (!ok) return;
    error = '';
    try {
      await databaseService.removeDatabase(db.id);
      await refresh();
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to remove database';
    }
  };

  const switchTo = async (db: DatabaseProfile) => {
    if (db.id === activeId || switching) return;
    switching = true;
    error = '';
    try {
      await onSwitch(db.id);
      await refresh();
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to switch database';
    } finally {
      switching = false;
    }
  };

  const pickLocationForNew = async () => {
    const suggested = addName.trim() ? `${addName.trim()}.sqlite3` : 'flashpad.sqlite3';
    const picked = await save({
      defaultPath: suggested,
      filters: [{ name: 'FlashPad database', extensions: ['sqlite3', 'db'] }],
    });
    if (picked) addPath = picked;
  };

  const pickExistingFile = async () => {
    const picked = await open({
      multiple: false,
      filters: [{ name: 'FlashPad database', extensions: ['sqlite3', 'db'] }],
    });
    if (typeof picked === 'string') {
      addPath = picked;
      if (!addName.trim()) {
        const stem = picked.split(/[/\\]/).pop()?.replace(/\.[^.]+$/, '') ?? '';
        addName = stem;
      }
    }
  };

  const confirmAdd = async () => {
    const name = addName.trim();
    if (!name || !addPath) return;
    addBusy = true;
    error = '';
    try {
      if (addMode === 'new') {
        await databaseService.createDatabase(name, addPath);
      } else {
        await databaseService.addExistingDatabase(name, addPath);
      }
      addMode = null;
      addName = '';
      addPath = '';
      await refresh();
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to add database';
    } finally {
      addBusy = false;
    }
  };

  const cancelAdd = () => {
    addMode = null;
    addName = '';
    addPath = '';
  };
</script>

<svelte:window on:keydown={handleKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="overlay" on:mousedown={handleOutsideClick}>
  <div class="panel" bind:this={panelEl} role="dialog" aria-modal="true" aria-label="Manage databases">
    <header>
      <h2>Databases</h2>
      <button class="close" on:click={onClose} aria-label="Close">
        <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
          <path d="M2 2l12 12M14 2L2 14" />
        </svg>
      </button>
    </header>

    {#if loading}
      <p class="hint">Loading…</p>
    {:else}
      <ul class="db-list">
        {#each databases as db (db.id)}
          <li class="db-row" class:active={db.id === activeId}>
            <div class="db-info">
              {#if renamingId === db.id}
                <input
                  class="rename-input"
                  bind:value={renameValue}
                  on:blur={() => commitRename(db.id)}
                  on:keydown={(e) => {
                    if (e.key === 'Enter') commitRename(db.id);
                    if (e.key === 'Escape') renamingId = null;
                  }}
                  autofocus
                />
              {:else}
                <span class="db-name">{db.name}</span>
                {#if db.id === activeId}
                  <span class="badge">Active</span>
                {/if}
              {/if}
              <span class="db-path" title={db.path}>{db.path}</span>
            </div>
            <div class="db-actions">
              {#if db.id !== activeId}
                <button class="btn" disabled={switching} on:click={() => switchTo(db)}>Switch</button>
              {/if}
              <button class="btn" on:click={() => startRename(db)}>Rename</button>
              {#if db.id !== activeId && databases.length > 1}
                <button class="btn danger" on:click={() => removeDatabase(db)}>Remove</button>
              {/if}
            </div>
          </li>
        {/each}
      </ul>
    {/if}

    {#if error}
      <p class="error">{error}</p>
    {/if}

    <div class="section">
      {#if addMode}
        <div class="add-form">
          <input class="name-input" placeholder="Name" bind:value={addName} />
          <div class="path-row">
            <input class="path-input" placeholder="No location chosen" readonly value={addPath} />
            <button class="btn" on:click={addMode === 'new' ? pickLocationForNew : pickExistingFile}>
              Choose…
            </button>
          </div>
          <div class="add-actions">
            <button class="btn" on:click={cancelAdd}>Cancel</button>
            <button class="btn primary" disabled={!addName.trim() || !addPath || addBusy} on:click={confirmAdd}>
              {addBusy ? 'Adding…' : 'Add'}
            </button>
          </div>
        </div>
      {:else}
        <div class="add-buttons">
          <button class="btn" on:click={() => (addMode = 'new')}>New Database…</button>
          <button class="btn" on:click={() => (addMode = 'existing')}>Add Existing…</button>
        </div>
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
    z-index: 1150;
  }

  .panel {
    width: min(460px, 92vw);
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
    font-size: 0.8rem;
    color: var(--muted);
  }

  .db-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .db-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    padding: 0.5rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: 0.5rem;
    background: var(--panel-2);
  }

  .db-row.active {
    border-color: var(--accent-soft, var(--border));
  }

  .db-info {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-width: 0;
  }

  .db-name {
    font-size: 0.82rem;
    color: var(--text);
    font-weight: 600;
  }

  .db-path {
    font-size: 0.7rem;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 240px;
  }

  .badge {
    display: inline-block;
    margin-left: 0.4rem;
    font-size: 0.65rem;
    color: var(--accent-soft, var(--muted));
    border: 1px solid var(--border);
    border-radius: 0.3rem;
    padding: 0.05rem 0.35rem;
    vertical-align: middle;
  }

  .rename-input {
    font-size: 0.82rem;
    border: 1px solid var(--border);
    border-radius: 0.3rem;
    background: var(--panel);
    color: var(--text);
    padding: 0.15rem 0.3rem;
  }

  .db-actions {
    display: flex;
    gap: 0.35rem;
    flex-shrink: 0;
  }

  .btn {
    border: 1px solid var(--border);
    border-radius: 0.35rem;
    background: var(--panel-2);
    color: var(--text);
    font-size: 0.75rem;
    padding: 0.3rem 0.6rem;
    cursor: pointer;
    white-space: nowrap;
  }

  .btn:hover:not(:disabled) {
    background: var(--accent-soft, var(--panel-2));
  }

  .btn:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .btn.danger {
    color: #ef4444;
    border-color: #ef4444;
  }

  .btn.primary {
    background: var(--accent-soft, var(--panel-2));
    font-weight: 600;
  }

  .section {
    margin-top: 0.6rem;
    padding-top: 0.6rem;
    border-top: 1px solid var(--border);
  }

  .add-buttons {
    display: flex;
    gap: 0.5rem;
  }

  .add-form {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .name-input,
  .path-input {
    font-size: 0.8rem;
    border: 1px solid var(--border);
    border-radius: 0.35rem;
    background: var(--panel-2);
    color: var(--text);
    padding: 0.35rem 0.5rem;
  }

  .path-input {
    color: var(--muted);
    flex: 1;
    min-width: 0;
  }

  .path-row {
    display: flex;
    gap: 0.4rem;
  }

  .add-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.4rem;
  }

  .error {
    margin: 0.4rem 0 0;
    font-size: 0.75rem;
    color: #ef4444;
  }
</style>

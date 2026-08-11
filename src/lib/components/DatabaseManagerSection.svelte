<script lang="ts">
  import { onMount } from 'svelte';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { DatabaseService, type DatabaseProfile } from '../services/databaseService';
  import { user } from '../stores/authStore';
  import { isCloudConfigured } from '../supabaseClient';
  import * as cloudService from '../services/cloudService';
  import { notebooks, members, refreshNotebooks, refreshMembers, resetCloudState } from '../stores/cloudStore';
  import type { NotebookSummary } from '../services/cloudService';
  import AuthPanel from './AuthPanel.svelte';

  export let onSwitch: (id: number) => Promise<void>;
  export let onSwitchCloud: (notebookId: string) => Promise<void>;
  export let onSignOut: () => Promise<void>;
  export let onRequestConfirm: (message: string) => Promise<boolean>;
  export let activeCloudNotebookId: string | null = null;

  const databaseService = new DatabaseService();

  // ---------- local databases ----------

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

  const refreshCloud = async () => {
    if (!isCloudConfigured() || !$user) return;
    await refreshNotebooks();
  };

  onMount(async () => {
    try {
      await Promise.all([refresh(), refreshCloud()]);
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to load databases';
    } finally {
      loading = false;
    }
  });

  // Re-fetch cloud notebooks whenever sign-in state changes (e.g. just
  // signed in from the AuthPanel below).
  $: if ($user) void refreshCloud();

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
    if ((db.id === activeId && activeCloudNotebookId == null) || switching) return;
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

  // ---------- cloud notebooks ----------

  let cloudSwitching = false;
  let cloudRenamingId: string | null = null;
  let cloudRenameValue = '';

  let showCreateForm = false;
  let newNotebookName = '';
  let createBusy = false;

  let joinCode = '';
  let joinBusy = false;
  let joinError = '';

  // The notebook currently expanded for sharing (invite code + member
  // management) - only one at a time.
  let sharingId: string | null = null;
  let activeInviteToken: string | null = null;
  let shareBusy = false;

  const startCloudRename = (nb: NotebookSummary) => {
    cloudRenamingId = nb.id;
    cloudRenameValue = nb.name;
  };

  const commitCloudRename = async (id: string) => {
    const name = cloudRenameValue.trim();
    cloudRenamingId = null;
    if (!name) return;
    try {
      await cloudService.renameNotebook(id, name);
      await refreshCloud();
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to rename notebook';
    }
  };

  const deleteNotebook = async (nb: NotebookSummary) => {
    const ok = await onRequestConfirm(`Delete "${nb.name}" for everyone? This removes all its notes permanently and cannot be undone.`);
    if (!ok) return;
    error = '';
    try {
      await cloudService.deleteNotebook(nb.id);
      await refreshCloud();
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to delete notebook';
    }
  };

  const leaveNotebook = async (nb: NotebookSummary) => {
    const ok = await onRequestConfirm(`Leave "${nb.name}"? You'll need a new invite code to rejoin.`);
    if (!ok) return;
    error = '';
    try {
      await cloudService.leaveNotebook(nb.id);
      await refreshCloud();
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to leave notebook';
    }
  };

  const switchToCloud = async (nb: NotebookSummary) => {
    if (nb.id === activeCloudNotebookId || cloudSwitching) return;
    cloudSwitching = true;
    error = '';
    try {
      await onSwitchCloud(nb.id);
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to switch notebook';
    } finally {
      cloudSwitching = false;
    }
  };

  const createNotebook = async () => {
    const name = newNotebookName.trim();
    if (!name) return;
    createBusy = true;
    error = '';
    try {
      await cloudService.createNotebook(name);
      newNotebookName = '';
      showCreateForm = false;
      await refreshCloud();
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to create notebook';
    } finally {
      createBusy = false;
    }
  };

  const joinViaCode = async () => {
    const token = joinCode.trim();
    if (!token) return;
    joinBusy = true;
    joinError = '';
    try {
      await cloudService.redeemInvite(token);
      joinCode = '';
      await refreshCloud();
    } catch (err) {
      joinError = err instanceof Error ? err.message : 'That invite code is invalid or has been revoked';
    } finally {
      joinBusy = false;
    }
  };

  const toggleSharing = async (nb: NotebookSummary) => {
    if (sharingId === nb.id) {
      sharingId = null;
      return;
    }
    sharingId = nb.id;
    activeInviteToken = null;
    error = '';
    try {
      await refreshMembers(nb.id);
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to load members';
    }
  };

  const generateInvite = async (notebookId: string) => {
    shareBusy = true;
    error = '';
    try {
      activeInviteToken = await cloudService.createInvite(notebookId);
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to create invite';
    } finally {
      shareBusy = false;
    }
  };

  const copyInvite = async (token: string) => {
    try {
      await navigator.clipboard.writeText(token);
    } catch {
      // Clipboard access can fail (permissions) - the code is still shown
      // on screen for a manual copy either way.
    }
  };

  const removeMember = async (notebookId: string, userId: string) => {
    const ok = await onRequestConfirm('Remove this member? They will lose access to the notebook.');
    if (!ok) return;
    error = '';
    try {
      await cloudService.removeMember(notebookId, userId);
      await refreshMembers(notebookId);
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to remove member';
    }
  };

  const doSignOut = async () => {
    error = '';
    try {
      await onSignOut();
      resetCloudState();
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to sign out';
    }
  };
</script>

{#if isCloudConfigured()}
  <div class="auth-status">
    {#if $user}
      <span class="signed-in-as">Signed in as {$user.email}</span>
      <button class="btn" on:click={doSignOut}>Sign out</button>
    {:else}
      <span class="signed-in-as">Sign in to sync notes and share a notebook with friends</span>
    {/if}
  </div>
  {#if !$user}
    <AuthPanel />
  {/if}
{/if}

{#if loading}
  <p class="hint">Loading…</p>
{:else}
  <ul class="db-list">
    {#each databases as db (db.id)}
      <li class="db-row" class:active={db.id === activeId && activeCloudNotebookId == null}>
        <div class="row-main">
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
              {#if db.id === activeId && activeCloudNotebookId == null}
                <span class="badge">Active</span>
              {/if}
              <span class="db-path" title={db.path}>{db.path}</span>
            {/if}
          </div>
          <div class="db-actions">
            {#if db.id !== activeId || activeCloudNotebookId != null}
              <button class="btn" disabled={switching} on:click={() => switchTo(db)}>Switch</button>
            {/if}
            <button class="btn" on:click={() => startRename(db)}>Rename</button>
            {#if (db.id !== activeId || activeCloudNotebookId != null) && databases.length > 1}
              <button class="btn danger" on:click={() => removeDatabase(db)}>Remove</button>
            {/if}
          </div>
        </div>
      </li>
    {/each}

    {#each $notebooks as nb (nb.id)}
      <li class="db-row" class:active={nb.id === activeCloudNotebookId}>
        <div class="row-main">
          <div class="db-info">
            {#if cloudRenamingId === nb.id}
              <input
                class="rename-input"
                bind:value={cloudRenameValue}
                on:blur={() => commitCloudRename(nb.id)}
                on:keydown={(e) => {
                  if (e.key === 'Enter') commitCloudRename(nb.id);
                  if (e.key === 'Escape') cloudRenamingId = null;
                }}
                autofocus
              />
            {:else}
              <span class="db-name">{nb.name}</span>
              {#if nb.id === activeCloudNotebookId}
                <span class="badge">Active</span>
              {/if}
              <span class="role-badge">Cloud · {nb.role}</span>
            {/if}
          </div>
          <div class="db-actions">
            {#if nb.id !== activeCloudNotebookId}
              <button class="btn" disabled={cloudSwitching} on:click={() => switchToCloud(nb)}>Switch</button>
            {/if}
            {#if nb.role === 'owner'}
              <button class="btn" on:click={() => startCloudRename(nb)}>Rename</button>
              <button class="btn" on:click={() => toggleSharing(nb)}>{sharingId === nb.id ? 'Close' : 'Share'}</button>
              <button class="btn danger" on:click={() => deleteNotebook(nb)}>Delete</button>
            {:else}
              <button class="btn danger" on:click={() => leaveNotebook(nb)}>Leave</button>
            {/if}
          </div>
        </div>

        {#if sharingId === nb.id && nb.role === 'owner'}
          <div class="share-panel">
            <button class="btn" disabled={shareBusy} on:click={() => generateInvite(nb.id)}>
              {shareBusy ? 'Generating…' : 'Generate invite code'}
            </button>
            {#if activeInviteToken}
              <div class="invite-row">
                <code class="invite-code">{activeInviteToken}</code>
                <button class="btn" on:click={() => copyInvite(activeInviteToken!)}>Copy</button>
              </div>
              <p class="hint">Share this code with a friend - they can redeem it from their own "Join via code" box below.</p>
            {/if}

            {#if $members.length}
              <span class="section-subtitle">Members</span>
              <ul class="member-list">
                {#each $members as member (member.userId)}
                  <li class="member-row">
                    <span class="member-id">{member.userId === $user?.id ? 'You' : member.userId.slice(0, 8)}</span>
                    <span class="role-badge">{member.role}</span>
                    {#if member.role !== 'owner'}
                      <button class="btn danger small" on:click={() => removeMember(nb.id, member.userId)}>Remove</button>
                    {/if}
                  </li>
                {/each}
              </ul>
            {/if}
          </div>
        {/if}
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
  {:else if showCreateForm}
    <div class="add-form">
      <input class="name-input" placeholder="Notebook name" bind:value={newNotebookName} />
      <div class="add-actions">
        <button class="btn" on:click={() => (showCreateForm = false)}>Cancel</button>
        <button class="btn primary" disabled={!newNotebookName.trim() || createBusy} on:click={createNotebook}>
          {createBusy ? 'Creating…' : 'Create'}
        </button>
      </div>
    </div>
  {:else}
    <div class="add-buttons">
      <button class="btn" on:click={() => (addMode = 'new')}>New Database…</button>
      <button class="btn" on:click={() => (addMode = 'existing')}>Add Existing…</button>
      {#if isCloudConfigured() && $user}
        <button class="btn" on:click={() => (showCreateForm = true)}>New Notebook…</button>
      {/if}
    </div>
  {/if}
</div>

{#if isCloudConfigured() && $user}
  <div class="section join-section">
    <span class="section-title">Join a shared notebook</span>
    <div class="join-row">
      <input class="name-input" placeholder="Paste invite code" bind:value={joinCode} />
      <button class="btn primary" disabled={!joinCode.trim() || joinBusy} on:click={joinViaCode}>
        {joinBusy ? 'Joining…' : 'Join'}
      </button>
    </div>
    {#if joinError}<p class="error">{joinError}</p>{/if}
  </div>
{/if}

<style>
  .hint {
    font-size: 0.8rem;
    color: var(--muted);
  }

  .auth-status {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    margin-bottom: 0.6rem;
  }

  .signed-in-as {
    font-size: 0.78rem;
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
    border: 1px solid var(--border);
    border-radius: 0.5rem;
    background: var(--panel-2);
    padding: 0.5rem 0.6rem;
  }

  .db-row.active {
    border-color: var(--accent-soft, var(--border));
  }

  .row-main {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .db-info {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    min-width: 0;
    flex-wrap: wrap;
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

  .badge,
  .role-badge {
    display: inline-block;
    font-size: 0.65rem;
    color: var(--accent-soft, var(--muted));
    border: 1px solid var(--border);
    border-radius: 0.3rem;
    padding: 0.05rem 0.35rem;
    vertical-align: middle;
  }

  .role-badge {
    text-transform: capitalize;
    color: var(--muted);
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
    flex-wrap: wrap;
    justify-content: flex-end;
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

  .btn.small {
    padding: 0.15rem 0.4rem;
    font-size: 0.7rem;
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

  .share-panel {
    margin-top: 0.5rem;
    padding-top: 0.5rem;
    border-top: 1px dashed var(--border);
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .invite-row {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }

  .invite-code {
    flex: 1;
    min-width: 0;
    font-size: 0.72rem;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 0.3rem;
    padding: 0.25rem 0.4rem;
    overflow-x: auto;
    white-space: nowrap;
  }

  .section-subtitle {
    font-size: 0.72rem;
    color: var(--muted);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.02em;
  }

  .member-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .member-row {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.75rem;
    color: var(--text);
  }

  .member-id {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .section {
    margin-top: 0.6rem;
    padding-top: 0.6rem;
    border-top: 1px solid var(--border);
  }

  .section-title {
    display: block;
    font-size: 0.72rem;
    color: var(--muted);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.02em;
    margin-bottom: 0.4rem;
  }

  .add-buttons {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
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

  .join-row {
    display: flex;
    gap: 0.4rem;
  }

  .join-row .name-input {
    flex: 1;
    min-width: 0;
  }

  .error {
    margin: 0.4rem 0 0;
    font-size: 0.75rem;
    color: #ef4444;
  }
</style>

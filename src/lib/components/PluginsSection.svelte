<script lang="ts">
  import { onMount } from 'svelte';
  import { openPath } from '@tauri-apps/plugin-opener';
  import { discoverPlugins, getPluginsDirPath, type DiscoveredPlugin } from '../plugins/pluginLoader';

  export let enabledPluginIds: string[];
  export let onSetPluginEnabled: (id: string, enabled: boolean) => Promise<void>;
  export let onReload: () => Promise<void>;

  let plugins: DiscoveredPlugin[] = [];
  let pluginsDirPath: string | null = null;
  let loading = true;
  let reloading = false;

  const refresh = async () => {
    loading = true;
    [plugins, pluginsDirPath] = await Promise.all([discoverPlugins(), getPluginsDirPath()]);
    loading = false;
  };

  onMount(refresh);

  const reload = async () => {
    reloading = true;
    await onReload();
    await refresh();
    reloading = false;
  };

  const openPluginsFolder = () => {
    if (pluginsDirPath) void openPath(pluginsDirPath);
  };
</script>

<div class="actions-row">
  <button class="btn" on:click={openPluginsFolder} disabled={!pluginsDirPath}>Open plugins folder</button>
  <button class="btn" on:click={reload} disabled={reloading}>{reloading ? 'Reloading…' : 'Reload plugins'}</button>
</div>

{#if loading}
  <p class="hint">Loading…</p>
{:else if plugins.length === 0}
  <p class="hint">
    No plugins found{pluginsDirPath ? ` in ${pluginsDirPath}` : ''}. Drop a plugin folder (a manifest.json + entry
    script) in there and reload.
  </p>
{:else}
  <ul class="plugin-list">
    {#each plugins as plugin (plugin.manifest.id)}
      <li class="plugin">
        <label class="row">
          <span>
            <span class="name">{plugin.manifest.name || plugin.manifest.id}</span>
            {#if plugin.manifest.version && plugin.manifest.version !== '?'}
              <span class="version">v{plugin.manifest.version}</span>
            {/if}
          </span>
          <input
            type="checkbox"
            checked={enabledPluginIds.includes(plugin.manifest.id)}
            disabled={!!plugin.error}
            on:change={(e) => onSetPluginEnabled(plugin.manifest.id, e.currentTarget.checked)}
          />
        </label>
        {#if plugin.manifest.description}
          <p class="description">{plugin.manifest.description}</p>
        {/if}
        {#if plugin.manifest.permissions?.length}
          <p class="permissions">Wants: {plugin.manifest.permissions.join(', ')}</p>
        {/if}
        {#if plugin.error}
          <p class="error">{plugin.error}</p>
        {/if}
      </li>
    {/each}
  </ul>
{/if}

<style>
  .actions-row {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 0.75rem;
  }

  .btn {
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    background: var(--panel-2);
    color: var(--text);
    font-size: 0.78rem;
    padding: 0.35rem 0.7rem;
    cursor: pointer;
  }

  .btn:hover:not(:disabled) {
    background: var(--border);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .plugin-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }

  .plugin {
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    padding: 0.55rem 0.7rem;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .name {
    font-size: 0.82rem;
    color: var(--text);
  }

  .version {
    margin-left: 0.4rem;
    font-size: 0.7rem;
    color: var(--muted);
  }

  .description,
  .permissions {
    margin: 0.3rem 0 0;
    font-size: 0.75rem;
    color: var(--muted);
  }

  .error {
    margin: 0.3rem 0 0;
    font-size: 0.75rem;
    color: #ef4444;
  }

  .hint {
    font-size: 0.78rem;
    color: var(--muted);
  }
</style>

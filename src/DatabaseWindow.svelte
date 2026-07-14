<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { emit, listen } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api/core';
  import { DatabaseService } from './lib/services/databaseService';
  import { SettingsService } from './lib/services/settingsService';
  import TitleBar from './lib/components/TitleBar.svelte';
  import ResizeHandles from './lib/components/ResizeHandles.svelte';
  import ConfirmDialog from './lib/components/ConfirmDialog.svelte';
  import DatabaseManagerSection from './lib/components/DatabaseManagerSection.svelte';

  const databaseService = new DatabaseService();
  const settingsService = new SettingsService();

  let confirmState: { message: string; resolve: (value: boolean) => void } | null = null;
  let unlistenTheme: (() => void) | undefined;

  // Each window is a separate document, so the theme this window renders
  // with has to be loaded and applied here too - it isn't inherited from
  // the main window. The window is created invisible (see
  // open_database_window in Rust) specifically so nothing shows before the
  // theme is applied and painted - the double rAF waits for the browser to
  // have actually painted with that theme, rather than revealing a flash of
  // the wrong (default dark) background first.
  onMount(() => {
    const reveal = () =>
      requestAnimationFrame(() =>
        requestAnimationFrame(() =>
          void invoke('show_utility_window', { label: 'database-manager' }).catch((err) =>
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
  });

  onDestroy(() => unlistenTheme?.());

  const requestConfirm = (message: string): Promise<boolean> => {
    return new Promise((resolve) => {
      confirmState = { message, resolve };
    });
  };

  const switchDatabase = async (id: number) => {
    const state = await databaseService.switchDatabase(id);
    await emit('database-changed', state);
  };

  const minimizeSelf = async () => {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    await getCurrentWindow().minimize();
  };

  const closeSelf = async () => {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    await getCurrentWindow().close();
  };
</script>

<svelte:head>
  <title>FlashPad Databases</title>
</svelte:head>

<div class="shell">
  <ResizeHandles />
  <TitleBar title="Databases" onMinimize={minimizeSelf} onClose={closeSelf} />

  <div class="content">
    <DatabaseManagerSection onSwitch={switchDatabase} onRequestConfirm={requestConfirm} />
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
</style>

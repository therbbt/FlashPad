import { exists, mkdir, readDir, readTextFile } from '@tauri-apps/plugin-fs';
import { BaseDirectory, appDataDir } from '@tauri-apps/api/path';
import { PluginSettingsService } from '../services/pluginSettingsService';
import { createPluginApi, resetPluginRegistry } from './pluginApi';

export interface PluginManifest {
  id: string;
  name: string;
  version: string;
  description?: string;
  permissions?: string[];
  entry: string;
}

export interface DiscoveredPlugin {
  manifest: PluginManifest;
  // The plugin's actual folder name on disk - not necessarily equal to
  // manifest.id, and the only thing safe to use when building a path to
  // the plugin's files (see loadEnabledPlugins).
  dir: string;
  // Set when the plugin's folder exists but its manifest couldn't be
  // read/parsed - still listed (so the user can see something's wrong)
  // rather than silently skipped.
  error?: string;
}

const PLUGINS_DIR = 'plugins';

const isTauriRuntime = () =>
  typeof window !== 'undefined' && Boolean((window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);

const pluginSettingsService = new PluginSettingsService();

// Ensures the plugins folder exists, so there's always somewhere for the
// user to drop a plugin into and "Open plugins folder" never fails on a
// fresh install. Safe to call every startup - mkdir with an existing
// target is a no-op here since we check first.
export async function ensurePluginsDirExists(): Promise<void> {
  if (!isTauriRuntime()) return;
  const dirExists = await exists(PLUGINS_DIR, { baseDir: BaseDirectory.AppData }).catch(() => false);
  if (!dirExists) await mkdir(PLUGINS_DIR, { baseDir: BaseDirectory.AppData, recursive: true });
}

export async function getPluginsDirPath(): Promise<string | null> {
  if (!isTauriRuntime()) return null;
  const base = await appDataDir();
  return `${base}/${PLUGINS_DIR}`;
}

// Lists every plugin folder and parses its manifest, regardless of
// enabled state - used to populate the Settings > Plugins list.
export async function discoverPlugins(): Promise<DiscoveredPlugin[]> {
  if (!isTauriRuntime()) return [];
  const dirExists = await exists(PLUGINS_DIR, { baseDir: BaseDirectory.AppData }).catch(() => false);
  if (!dirExists) return [];

  const entries = await readDir(PLUGINS_DIR, { baseDir: BaseDirectory.AppData });
  const results: DiscoveredPlugin[] = [];
  for (const entry of entries) {
    if (!entry.isDirectory) continue;
    try {
      const manifestText = await readTextFile(`${PLUGINS_DIR}/${entry.name}/manifest.json`, {
        baseDir: BaseDirectory.AppData,
      });
      const manifest = JSON.parse(manifestText) as PluginManifest;
      if (!manifest.id || !manifest.entry) throw new Error('manifest.json is missing "id" or "entry"');
      results.push({ manifest, dir: entry.name });
    } catch (err) {
      results.push({
        manifest: { id: entry.name, name: entry.name, version: '?', entry: '' },
        dir: entry.name,
        error: err instanceof Error ? err.message : 'Failed to read manifest.json',
      });
    }
  }
  return results;
}

// Loads and activates every plugin whose id is in `enabledPluginIds`.
// Safe to call again (toggling a plugin, "Reload plugins") - clears every
// previous contribution first, since there's no per-plugin unregister yet
// (see pluginApi.ts); a still-enabled plugin just re-registers its items
// when reactivated right after.
export async function loadEnabledPlugins(enabledPluginIds: string[]): Promise<void> {
  resetPluginRegistry();
  if (!isTauriRuntime() || enabledPluginIds.length === 0) return;

  const discovered = await discoverPlugins();
  for (const { manifest, dir, error } of discovered) {
    if (error || !enabledPluginIds.includes(manifest.id)) continue;
    try {
      const entryText = await readTextFile(`${PLUGINS_DIR}/${dir}/${manifest.entry}`, {
        baseDir: BaseDirectory.AppData,
      });
      // The standard technique for loading arbitrary runtime JS as a real
      // ES module in a Vite-bundled app, without a rebuild: wrap the
      // source in a Blob and import its object URL. @vite-ignore stops
      // Vite from trying to statically analyze this dynamic import target.
      const blob = new Blob([entryText], { type: 'text/javascript' });
      const url = URL.createObjectURL(blob);
      try {
        const module = await import(/* @vite-ignore */ url);
        const plugin = module.default;
        if (!plugin?.activate) throw new Error('plugin has no default export with an activate() function');
        plugin.activate(createPluginApi(manifest.id, pluginSettingsService));
      } finally {
        URL.revokeObjectURL(url);
      }
    } catch (err) {
      console.error(`[flashpad] failed to load plugin "${manifest.id}"`, err);
    }
  }
}

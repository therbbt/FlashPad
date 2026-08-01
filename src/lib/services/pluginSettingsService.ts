// Per-plugin key/value settings, namespaced by plugin id so plugins can't
// collide with each other or with the app's own settings
// (settingsService.ts). Same plain-localStorage approach as the rest of
// the app's settings for now - not encrypted, so plugins shouldn't be
// told to store real secrets (like a GitHub token) here without the user
// understanding that. A future OS-keychain-backed store could replace
// the storage underneath this same get/set API without plugins noticing.

const STORAGE_KEY = 'flashpad.plugin-settings';

type PluginSettingsStore = Record<string, Record<string, unknown>>;

const readAll = (): PluginSettingsStore => {
  if (typeof window === 'undefined') return {};
  const stored = window.localStorage.getItem(STORAGE_KEY);
  if (!stored) return {};
  try {
    return JSON.parse(stored) as PluginSettingsStore;
  } catch {
    return {};
  }
};

const writeAll = (store: PluginSettingsStore): void => {
  if (typeof window === 'undefined') return;
  window.localStorage.setItem(STORAGE_KEY, JSON.stringify(store));
};

export class PluginSettingsService {
  get(pluginId: string, key: string): unknown {
    return readAll()[pluginId]?.[key];
  }

  async set(pluginId: string, key: string, value: unknown): Promise<void> {
    const store = readAll();
    store[pluginId] = { ...store[pluginId], [key]: value };
    writeAll(store);
  }

  // Called when a plugin is uninstalled (removed from disk) so its
  // leftover settings don't accumulate forever.
  clear(pluginId: string): void {
    const store = readAll();
    delete store[pluginId];
    writeAll(store);
  }
}

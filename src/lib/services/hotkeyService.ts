import { invoke } from '@tauri-apps/api/core';

const isTauriRuntime = () => typeof window !== 'undefined' && Boolean((window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);

const DEFAULT_HOTKEY = 'Alt+S';

// The global hotkey is registered and persisted entirely in Rust (see
// src-tauri/src/main.rs) so it works instantly even while the window is
// hidden, and is already active by the time the frontend loads - routing
// this through JS at startup would require waking up a potentially-suspended
// webview for what should be an instant toggle.
export class HotkeyService {
  async get(): Promise<string> {
    if (!isTauriRuntime()) return DEFAULT_HOTKEY;
    return await invoke<string>('get_hotkey');
  }

  async set(hotkey: string): Promise<void> {
    if (!isTauriRuntime()) return;
    await invoke('set_hotkey', { hotkey });
  }
}

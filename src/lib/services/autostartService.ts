import { isEnabled as isAutostartEnabled, enable, disable } from '@tauri-apps/plugin-autostart';

const isTauriRuntime = () => typeof window !== 'undefined' && Boolean((window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);

export class AutostartService {
  async isEnabled(): Promise<boolean> {
    if (!isTauriRuntime()) return false;
    return await isAutostartEnabled();
  }

  async setEnabled(value: boolean): Promise<void> {
    if (!isTauriRuntime()) return;
    if (value) {
      await enable();
    } else {
      await disable();
    }
  }
}

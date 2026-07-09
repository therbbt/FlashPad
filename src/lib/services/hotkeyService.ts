import { register, unregisterAll } from '@tauri-apps/plugin-global-shortcut';
import { getCurrentWindow } from '@tauri-apps/api/window';

const isTauriRuntime = () =>
  typeof window !== 'undefined' && Boolean((window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);

export class HotkeyService {
  private current: string | null = null;

  async register(hotkey: string): Promise<void> {
    if (!isTauriRuntime() || this.current === hotkey) return;

    await unregisterAll();
    await register(hotkey, (event) => {
      if (event.state !== 'Pressed') return;
      const win = getCurrentWindow();
      void win.show();
      void win.setFocus();
    });
    this.current = hotkey;
  }
}

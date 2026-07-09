export interface FlashPadSettings {
  hotkey: string;
  timestampFormat: 'classic' | 'markdown' | 'simple';
  theme: 'dark' | 'light';
}

const STORAGE_KEY = 'flashpad.settings';

export class SettingsService {
  private cached: FlashPadSettings | null = null;

  async load(): Promise<FlashPadSettings> {
    if (this.cached) return this.cached;
    const stored = typeof window !== 'undefined' ? window.localStorage.getItem(STORAGE_KEY) : null;
    const parsed = stored ? (JSON.parse(stored) as Partial<FlashPadSettings>) : {};
    this.cached = {
      hotkey: parsed.hotkey ?? 'Ctrl+Alt+N',
      timestampFormat: parsed.timestampFormat ?? 'classic',
      theme: parsed.theme ?? 'dark',
    };
    return this.cached;
  }

  getCached(): FlashPadSettings {
    return this.cached ?? { hotkey: 'Ctrl+Alt+N', timestampFormat: 'classic', theme: 'dark' };
  }

  async save(settings: Partial<FlashPadSettings>): Promise<void> {
    const current = await this.load();
    this.cached = { ...current, ...settings };
    if (typeof window !== 'undefined') {
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(this.cached));
    }
  }

  async saveTheme(theme: FlashPadSettings['theme']): Promise<void> {
    await this.save({ theme });
  }

  renderTimestamp(format: FlashPadSettings['timestampFormat']): string {
    const stamp = new Date().toISOString().slice(0, 19).replace('T', ' ');
    switch (format) {
      case 'markdown':
        return `## ${stamp}\n`;
      case 'simple':
        return `${stamp}\n`;
      case 'classic':
      default:
        return `--------------------------------------------------\n${stamp}\n--------------------------------------------------\n`;
    }
  }
}

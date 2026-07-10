export interface FlashPadSettings {
  hotkey: string;
  timestampFormat: 'classic' | 'markdown' | 'simple';
  theme: 'dark' | 'light';
}

const STORAGE_KEY = 'flashpad.settings';

// There's no settings UI yet to let a user actually choose this, so it isn't
// read from or written to persisted storage below - only theme/timestampFormat
// are. Once a real hotkey picker exists, persist it through an explicit save
// action instead of letting it ride along with unrelated setting changes;
// otherwise whatever this constant happened to be at the time of someone's
// last save (e.g. toggling theme) gets baked in forever and silently
// overrides every future change to this default.
const DEFAULT_HOTKEY = 'Alt+S';

export class SettingsService {
  private cached: FlashPadSettings | null = null;

  async load(): Promise<FlashPadSettings> {
    if (this.cached) return this.cached;
    const stored = typeof window !== 'undefined' ? window.localStorage.getItem(STORAGE_KEY) : null;
    const parsed = stored ? (JSON.parse(stored) as Partial<FlashPadSettings>) : {};
    this.cached = {
      hotkey: DEFAULT_HOTKEY,
      timestampFormat: parsed.timestampFormat ?? 'classic',
      theme: parsed.theme ?? 'dark',
    };
    return this.cached;
  }

  getCached(): FlashPadSettings {
    return this.cached ?? { hotkey: DEFAULT_HOTKEY, timestampFormat: 'classic', theme: 'dark' };
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

import { DEFAULT_DARK_PALETTE_ID, DEFAULT_LIGHT_PALETTE_ID } from '../theme/palettes';

export interface FlashPadSettings {
  theme: 'dark' | 'light';
  lightPaletteId: string;
  darkPaletteId: string;
  // Version the user last dismissed the update notification for ("Not
  // now"), so the check on the next startup doesn't nag about the same
  // release again - only a newer version reopens the toast.
  dismissedUpdateVersion: string | null;
}

const STORAGE_KEY = 'flashpad.settings';

const DEFAULTS: FlashPadSettings = {
  theme: 'dark',
  lightPaletteId: DEFAULT_LIGHT_PALETTE_ID,
  darkPaletteId: DEFAULT_DARK_PALETTE_ID,
  dismissedUpdateVersion: null,
};

export class SettingsService {
  private cached: FlashPadSettings | null = null;

  async load(): Promise<FlashPadSettings> {
    if (this.cached) return this.cached;
    const stored = typeof window !== 'undefined' ? window.localStorage.getItem(STORAGE_KEY) : null;
    const parsed = stored ? (JSON.parse(stored) as Partial<FlashPadSettings>) : {};
    this.cached = {
      theme: parsed.theme ?? DEFAULTS.theme,
      lightPaletteId: parsed.lightPaletteId ?? DEFAULTS.lightPaletteId,
      darkPaletteId: parsed.darkPaletteId ?? DEFAULTS.darkPaletteId,
      dismissedUpdateVersion: parsed.dismissedUpdateVersion ?? DEFAULTS.dismissedUpdateVersion,
    };
    return this.cached;
  }

  getCached(): FlashPadSettings {
    return this.cached ?? DEFAULTS;
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

  async saveLightPalette(lightPaletteId: string): Promise<void> {
    await this.save({ lightPaletteId });
  }

  async saveDarkPalette(darkPaletteId: string): Promise<void> {
    await this.save({ darkPaletteId });
  }

  async saveDismissedUpdateVersion(version: string): Promise<void> {
    await this.save({ dismissedUpdateVersion: version });
  }
}

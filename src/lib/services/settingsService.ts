import { DEFAULT_DARK_PALETTE_ID, DEFAULT_LIGHT_PALETTE_ID } from '../theme/palettes';

export interface FlashPadSettings {
  theme: 'dark' | 'light';
  lightPaletteId: string;
  darkPaletteId: string;
  // Version the user last dismissed the update notification for ("Not
  // now"), so the check on the next startup doesn't nag about the same
  // release again - only a newer version reopens the toast.
  dismissedUpdateVersion: string | null;
  // Version the app was running as of the last successful startup check -
  // NOT the same thing as dismissedUpdateVersion (that's about an update
  // offered but not yet installed; this is about a version already
  // running). Comparing this against the current build's version on
  // startup is how the "What's new" dialog knows the app was just updated,
  // as opposed to a fresh install or an unchanged version. Only updated
  // once the What's new dialog has actually been shown (or shown to be
  // unnecessary) - see checkForWhatsNew in App.svelte - so a transient
  // failure to fetch that version's notes retries on the next launch
  // instead of silently skipping it forever.
  lastSeenVersion: string | null;
}

const STORAGE_KEY = 'flashpad.settings';

const DEFAULTS: FlashPadSettings = {
  theme: 'dark',
  lightPaletteId: DEFAULT_LIGHT_PALETTE_ID,
  darkPaletteId: DEFAULT_DARK_PALETTE_ID,
  dismissedUpdateVersion: null,
  lastSeenVersion: null,
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
      lastSeenVersion: parsed.lastSeenVersion ?? DEFAULTS.lastSeenVersion,
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

  async saveLastSeenVersion(version: string): Promise<void> {
    await this.save({ lastSeenVersion: version });
  }
}

import { invoke } from '@tauri-apps/api/core';
import type { BackupSettings } from './databaseService';

export interface BackupInfo {
  path: string;
  filename: string;
  sizeBytes: number;
}

export class BackupService {
  async createNow(): Promise<BackupInfo> {
    return await invoke<BackupInfo>('create_backup_now');
  }

  async list(): Promise<BackupInfo[]> {
    return await invoke<BackupInfo[]>('list_backups');
  }

  async getSettings(): Promise<BackupSettings> {
    return await invoke<BackupSettings>('get_backup_settings');
  }

  async setRetentionCount(retentionCount: number): Promise<void> {
    await invoke('set_backup_settings', { retentionCount });
  }

  async exportTo(destPath: string): Promise<void> {
    await invoke('export_database', { destPath });
  }

  // The backend takes an automatic pre-import safety backup before touching
  // anything and validates the file is really a FlashPad database - the
  // frontend's job is only to warn the user this replaces their current
  // data (see ConfirmDialog usage in App.svelte) before calling this.
  async importFrom(sourcePath: string): Promise<void> {
    await invoke('import_database', { sourcePath });
  }
}

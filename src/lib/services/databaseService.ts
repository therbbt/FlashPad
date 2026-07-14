import { invoke } from '@tauri-apps/api/core';

const isTauriRuntime = () => typeof window !== 'undefined' && Boolean((window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);

export interface DatabaseProfile {
  id: number;
  name: string;
  path: string;
  createdAt: string;
}

export interface BackupSettings {
  retentionCount: number;
}

export interface AppState {
  activeDatabaseId: number;
  databases: DatabaseProfile[];
  ready: boolean;
  error: string | null;
  syncWarning: string | null;
  backup: BackupSettings;
}

// All state here (which databases exist, which is active, whether the
// active one is actually reachable) lives in Rust - see
// src-tauri/src/profiles.rs - since it has to survive a completely
// unreachable/corrupt database and be readable before any SQLite connection
// exists at all (flashpad.config.json, not the database itself).
export class DatabaseService {
  async init(): Promise<void> {
    return;
  }

  async getAppState(): Promise<AppState | null> {
    if (!isTauriRuntime()) return null;
    return await invoke<AppState>('get_app_state');
  }

  async listDatabases(): Promise<DatabaseProfile[]> {
    if (!isTauriRuntime()) return [];
    return await invoke<DatabaseProfile[]>('list_databases');
  }

  async createDatabase(name: string, path: string): Promise<DatabaseProfile> {
    return await invoke<DatabaseProfile>('create_database', { name, path });
  }

  async addExistingDatabase(name: string, path: string): Promise<DatabaseProfile> {
    return await invoke<DatabaseProfile>('add_existing_database', { name, path });
  }

  async renameDatabase(id: number, name: string): Promise<void> {
    await invoke('rename_database', { id, name });
  }

  async removeDatabase(id: number): Promise<void> {
    await invoke('remove_database', { id });
  }

  async switchDatabase(id: number): Promise<AppState> {
    return await invoke<AppState>('switch_database', { id });
  }

  // Closes and reopens the active database's connection from disk, without
  // changing which profile is active - lets the user pick up changes
  // written by another device/process (e.g. a sync client) without
  // restarting the app. Like switchDatabase, this can resolve successfully
  // even if the file turned out to be unreachable - check `.ready` on the
  // result rather than assuming success from the absence of a thrown error.
  async reloadDatabase(): Promise<AppState> {
    return await invoke<AppState>('reload_database');
  }

  async setDatabasePath(newPath: string): Promise<DatabaseProfile> {
    return await invoke<DatabaseProfile>('set_database_path', { newPath });
  }
}

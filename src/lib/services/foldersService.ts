import { invoke } from '@tauri-apps/api/core';

export interface FolderRecord {
  id: number;
  name: string;
  parentId: number | null;
  createdAt: string;
  updatedAt: string;
}

const STORAGE_KEY = 'flashpad.folders';

const isTauriRuntime = () => typeof window !== 'undefined' && Boolean((window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);

const readFallback = (): FolderRecord[] => {
  if (typeof window === 'undefined') return [];
  const raw = window.localStorage.getItem(STORAGE_KEY);
  return raw ? (JSON.parse(raw) as FolderRecord[]) : [];
};

const writeFallback = (folders: FolderRecord[]) => {
  if (typeof window !== 'undefined') {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(folders));
  }
};

export class FoldersService {
  async list(): Promise<FolderRecord[]> {
    if (!isTauriRuntime()) {
      return readFallback();
    }
    return await invoke<FolderRecord[]>('list_folders');
  }

  async create(name: string, parentId: number | null): Promise<FolderRecord> {
    if (!isTauriRuntime()) {
      const folder: FolderRecord = {
        id: Date.now(),
        name,
        parentId,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      };
      writeFallback([...readFallback(), folder]);
      return folder;
    }
    return await invoke<FolderRecord>('create_folder', { folder: { name, parentId } });
  }

  async rename(id: number, name: string): Promise<FolderRecord> {
    if (!isTauriRuntime()) {
      const folders = readFallback().map((item) => (item.id === id ? { ...item, name, updatedAt: new Date().toISOString() } : item));
      writeFallback(folders);
      return folders.find((item) => item.id === id) as FolderRecord;
    }
    return await invoke<FolderRecord>('rename_folder', { id, name });
  }

  async move(id: number, parentId: number | null): Promise<FolderRecord> {
    if (!isTauriRuntime()) {
      const folders = readFallback().map((item) => (item.id === id ? { ...item, parentId, updatedAt: new Date().toISOString() } : item));
      writeFallback(folders);
      return folders.find((item) => item.id === id) as FolderRecord;
    }
    return await invoke<FolderRecord>('move_folder', { id, parentId });
  }

  async delete(id: number): Promise<void> {
    if (!isTauriRuntime()) {
      const descendantIds = this.collectDescendantIds(readFallback(), id);
      writeFallback(readFallback().filter((item) => item.id !== id && !descendantIds.has(item.id)));
      return;
    }
    await invoke('delete_folder', { id });
  }

  private collectDescendantIds(folders: FolderRecord[], rootId: number): Set<number> {
    const ids = new Set<number>();
    const queue = [rootId];
    while (queue.length) {
      const current = queue.pop()!;
      for (const folder of folders) {
        if (folder.parentId === current && !ids.has(folder.id)) {
          ids.add(folder.id);
          queue.push(folder.id);
        }
      }
    }
    return ids;
  }
}

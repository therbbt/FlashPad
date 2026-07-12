import { invoke } from '@tauri-apps/api/core';

export interface NoteRecord {
  id: number;
  title: string;
  content: string;
  parentId: number | null;
  createdAt: string;
  updatedAt: string;
  isMarkdown: boolean;
  isLocked: boolean;
  sortOrder: number;
}

const STORAGE_KEY = 'flashpad.notes';

const isTauriRuntime = () => typeof window !== 'undefined' && Boolean((window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);

const readFallback = (): NoteRecord[] => {
  if (typeof window === 'undefined') return [];
  const raw = window.localStorage.getItem(STORAGE_KEY);
  return raw ? (JSON.parse(raw) as NoteRecord[]) : [];
};

const writeFallback = (notes: NoteRecord[]) => {
  if (typeof window !== 'undefined') {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(notes));
  }
};

export class NotesService {
  async list(): Promise<NoteRecord[]> {
    if (!isTauriRuntime()) {
      return readFallback();
    }
    return await invoke<NoteRecord[]>('list_notes');
  }

  async create(payload: { title?: string; content?: string; parentId?: number | null; isMarkdown?: boolean } = {}): Promise<NoteRecord> {
    if (!isTauriRuntime()) {
      const parentId = payload.parentId ?? null;
      const existing = readFallback();
      const siblingOrders = existing.filter((n) => n.parentId === parentId).map((n) => n.sortOrder);
      const note: NoteRecord = {
        id: Date.now(),
        title: payload.title ?? 'Untitled',
        content: payload.content ?? '',
        parentId,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
        isMarkdown: payload.isMarkdown ?? false,
        isLocked: false,
        sortOrder: siblingOrders.length ? Math.max(...siblingOrders) + 1 : 0,
      };
      const notes = [...existing, note];
      writeFallback(notes);
      return note;
    }
    return await invoke<NoteRecord>('create_note', {
      note: { title: payload.title, content: payload.content, parentId: payload.parentId ?? null, isMarkdown: payload.isMarkdown ?? false },
    });
  }

  async save(note: { id: number; title?: string; content?: string; isMarkdown?: boolean; isLocked?: boolean }): Promise<NoteRecord> {
    if (!isTauriRuntime()) {
      const notes = readFallback().map((item) => (item.id === note.id ? { ...item, ...note, updatedAt: new Date().toISOString() } : item));
      writeFallback(notes);
      return notes.find((item) => item.id === note.id) as NoteRecord;
    }
    return await invoke<NoteRecord>('update_note', { note });
  }

  async delete(id: number): Promise<void> {
    if (!isTauriRuntime()) {
      writeFallback(readFallback().filter((note) => note.id !== id));
      return;
    }
    await invoke('delete_note', { id });
  }

  async move(id: number, parentId: number | null): Promise<NoteRecord> {
    if (!isTauriRuntime()) {
      const existing = readFallback();
      const siblingOrders = existing.filter((n) => n.parentId === parentId && n.id !== id).map((n) => n.sortOrder);
      const sortOrder = siblingOrders.length ? Math.max(...siblingOrders) + 1 : 0;
      const notes = existing.map((item) => (item.id === id ? { ...item, parentId, sortOrder, updatedAt: new Date().toISOString() } : item));
      writeFallback(notes);
      return notes.find((item) => item.id === id) as NoteRecord;
    }
    return await invoke<NoteRecord>('move_note', { id, parentId });
  }

  async duplicate(id: number): Promise<NoteRecord> {
    if (!isTauriRuntime()) {
      const existing = readFallback();
      const source = existing.find((item) => item.id === id);
      if (!source) throw new Error('Note not found');
      const siblingOrders = existing.filter((n) => n.parentId === source.parentId).map((n) => n.sortOrder);
      const copy: NoteRecord = {
        ...source,
        id: Date.now(),
        title: `${source.title} (copy)`,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
        isLocked: false,
        sortOrder: siblingOrders.length ? Math.max(...siblingOrders) + 1 : 0,
      };
      writeFallback([...existing, copy]);
      return copy;
    }
    return await invoke<NoteRecord>('duplicate_note', { id });
  }

  async reorder(id: number, parentId: number | null, beforeId: number | null): Promise<NoteRecord> {
    if (!isTauriRuntime()) {
      const existing = readFallback();
      const siblings = existing
        .filter((n) => n.parentId === parentId && n.id !== id)
        .sort((a, b) => a.sortOrder - b.sortOrder)
        .map((n) => n.id);
      const insertAt = beforeId != null && siblings.includes(beforeId) ? siblings.indexOf(beforeId) : siblings.length;
      siblings.splice(insertAt, 0, id);
      const orderById = new Map(siblings.map((sid, index) => [sid, index]));
      const notes = existing.map((item) =>
        orderById.has(item.id)
          ? { ...item, parentId, sortOrder: orderById.get(item.id)!, updatedAt: item.id === id ? new Date().toISOString() : item.updatedAt }
          : item,
      );
      writeFallback(notes);
      return notes.find((item) => item.id === id) as NoteRecord;
    }
    return await invoke<NoteRecord>('reorder_note', { id, parentId, beforeId });
  }

  async search(query: string): Promise<NoteRecord[]> {
    const notes = await this.list();
    if (!query.trim()) return notes;
    const haystack = query.toLowerCase();
    return notes.filter((note) => `${note.title} ${note.content}`.toLowerCase().includes(haystack));
  }
}

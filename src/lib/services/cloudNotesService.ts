import { supabase } from '../supabaseClient';
import type { NoteRecord, NotesBackend } from './notesService';

const COLUMNS = 'id, title, content, parent_id, created_at, updated_at, is_markdown, is_locked, sort_order, show_line_numbers, language, is_editor_mode';

interface NoteRow {
  id: number;
  title: string;
  content: string;
  parent_id: number | null;
  created_at: string;
  updated_at: string;
  is_markdown: boolean;
  is_locked: boolean;
  sort_order: number;
  show_line_numbers: boolean;
  language: string | null;
  is_editor_mode: boolean;
}

const rowToNote = (row: NoteRow): NoteRecord => ({
  id: row.id,
  title: row.title,
  content: row.content,
  parentId: row.parent_id,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
  isMarkdown: row.is_markdown,
  isLocked: row.is_locked,
  sortOrder: row.sort_order,
  showLineNumbers: row.show_line_numbers,
  language: row.language,
  isEditorMode: row.is_editor_mode,
});

// Ports src-tauri/src/notes.rs's Tauri commands to Supabase, scoped to a
// single cloud notebook - see supabase/schema.sql for the matching table/
// RLS/trigger definitions. Every UPDATE's column list below is deliberately
// shaped to match the Rust commands' own SQL exactly, because
// notes_touch_updated_at in schema.sql (not this code) is what decides
// whether updated_at gets bumped, based on which columns are named in the
// UPDATE's SET clause - see that trigger's comment for the full mapping.
export class CloudNotesService implements NotesBackend {
  constructor(private readonly notebookId: string) {}

  private client() {
    if (!supabase) throw new Error('Cloud notebooks are not configured');
    return supabase;
  }

  private async findNote(id: number): Promise<NoteRecord> {
    const { data, error } = await this.client()
      .from('notes')
      .select(COLUMNS)
      .eq('notebook_id', this.notebookId)
      .eq('id', id)
      .single();
    if (error) throw new Error(error.message);
    return rowToNote(data as unknown as NoteRow);
  }

  // Mirrors next_sort_order in notes.rs: one past the highest sort_order
  // among the target parent's current children.
  private async nextSortOrder(parentId: number | null): Promise<number> {
    let query = this.client()
      .from('notes')
      .select('sort_order')
      .eq('notebook_id', this.notebookId)
      .order('sort_order', { ascending: false })
      .limit(1);
    query = parentId === null ? query.is('parent_id', null) : query.eq('parent_id', parentId);
    const { data, error } = await query;
    if (error) throw new Error(error.message);
    const rows = data as { sort_order: number }[];
    return (rows.length ? rows[0].sort_order : -1) + 1;
  }

  // Mirrors check_no_cycle in notes.rs: walks the parent chain from
  // parentId back up to the root, rejecting a move that would place id
  // inside itself or one of its own descendants.
  private async checkNoCycle(id: number, parentId: number | null): Promise<void> {
    if (parentId === id) throw new Error('A note cannot be moved into itself');
    let cursor = parentId;
    while (cursor !== null) {
      if (cursor === id) throw new Error('Cannot move a note into one of its own subnotes');
      const { data, error } = await this.client()
        .from('notes')
        .select('parent_id')
        .eq('notebook_id', this.notebookId)
        .eq('id', cursor)
        .single();
      if (error) throw new Error(error.message);
      cursor = (data as { parent_id: number | null }).parent_id;
    }
  }

  async list(): Promise<NoteRecord[]> {
    const { data, error } = await this.client()
      .from('notes')
      .select(COLUMNS)
      .eq('notebook_id', this.notebookId)
      .order('updated_at', { ascending: false });
    if (error) throw new Error(error.message);
    return (data as unknown as NoteRow[]).map(rowToNote);
  }

  async create(payload: { title?: string; content?: string; parentId?: number | null; isMarkdown?: boolean; isEditorMode?: boolean }): Promise<NoteRecord> {
    const parentId = payload.parentId ?? null;
    const sortOrder = await this.nextSortOrder(parentId);
    const { data, error } = await this.client()
      .from('notes')
      .insert({
        notebook_id: this.notebookId,
        title: payload.title ?? 'Untitled',
        content: payload.content ?? '',
        parent_id: parentId,
        is_markdown: payload.isMarkdown ?? false,
        sort_order: sortOrder,
        is_editor_mode: payload.isEditorMode ?? false,
      })
      .select(COLUMNS)
      .single();
    if (error) throw new Error(error.message);
    return rowToNote(data as unknown as NoteRow);
  }

  // Reject rule mirrors update_note's own comment: defense-in-depth against
  // a stray autosave racing the lock, not a hard security boundary - RLS
  // can't distinguish "this write came from the checklist-toggle path" from
  // a general save, so this check lives client-side only, same as the
  // Rust version's own framing of it.
  async save(note: {
    id: number;
    title?: string;
    content?: string;
    isMarkdown?: boolean;
    isLocked?: boolean;
    showLineNumbers?: boolean;
    language?: string;
    isEditorMode?: boolean;
  }): Promise<NoteRecord> {
    const existing = await this.findNote(note.id);
    const isLocked = note.isLocked ?? existing.isLocked;

    if (
      existing.isLocked &&
      isLocked &&
      ((note.title !== undefined && note.title !== existing.title) ||
        (note.content !== undefined && note.content !== existing.content))
    ) {
      throw new Error('Note is locked');
    }

    const title = note.title ?? existing.title;
    const content = note.content ?? existing.content;
    const isMarkdown = note.isMarkdown ?? existing.isMarkdown;
    const showLineNumbers = note.showLineNumbers ?? existing.showLineNumbers;
    const isEditorMode = note.isEditorMode ?? existing.isEditorMode;
    // Same two-state convention as notes.rs's update_note: undefined = leave
    // unchanged, '' = explicitly reset to auto-detect (NULL), nonempty = pin.
    const language = note.language === undefined ? existing.language : note.language === '' ? null : note.language;

    const { data, error } = await this.client()
      .from('notes')
      .update({
        title,
        content,
        is_markdown: isMarkdown,
        is_locked: isLocked,
        show_line_numbers: showLineNumbers,
        language,
        is_editor_mode: isEditorMode,
      })
      .eq('notebook_id', this.notebookId)
      .eq('id', note.id)
      .select(COLUMNS)
      .single();
    if (error) throw new Error(error.message);
    return rowToNote(data as unknown as NoteRow);
  }

  // Deliberately no lock check - same narrow exception as
  // save_checklist_toggle in notes.rs, only ever call from the
  // checkbox-toggle path.
  async saveChecklistToggle(id: number, content: string): Promise<NoteRecord> {
    const { data, error } = await this.client()
      .from('notes')
      .update({ content })
      .eq('notebook_id', this.notebookId)
      .eq('id', id)
      .select(COLUMNS)
      .single();
    if (error) throw new Error(error.message);
    return rowToNote(data as unknown as NoteRow);
  }

  async delete(id: number): Promise<void> {
    const { error } = await this.client().from('notes').delete().eq('notebook_id', this.notebookId).eq('id', id);
    if (error) throw new Error(error.message);
  }

  async move(id: number, parentId: number | null): Promise<NoteRecord> {
    await this.checkNoCycle(id, parentId);
    const sortOrder = await this.nextSortOrder(parentId);
    const { data, error } = await this.client()
      .from('notes')
      .update({ parent_id: parentId, sort_order: sortOrder })
      .eq('notebook_id', this.notebookId)
      .eq('id', id)
      .select(COLUMNS)
      .single();
    if (error) throw new Error(error.message);
    return rowToNote(data as unknown as NoteRow);
  }

  async duplicate(id: number): Promise<NoteRecord> {
    const source = await this.findNote(id);
    const sortOrder = await this.nextSortOrder(source.parentId);
    const { data, error } = await this.client()
      .from('notes')
      .insert({
        notebook_id: this.notebookId,
        title: `${source.title} (copy)`,
        content: source.content,
        parent_id: source.parentId,
        is_markdown: source.isMarkdown,
        is_locked: false,
        sort_order: sortOrder,
        show_line_numbers: source.showLineNumbers,
        language: source.language,
        is_editor_mode: source.isEditorMode,
      })
      .select(COLUMNS)
      .single();
    if (error) throw new Error(error.message);
    return rowToNote(data as unknown as NoteRow);
  }

  // Mirrors reorder_note in notes.rs, including issuing two DIFFERENT
  // update shapes: the dragged note's row includes parent_id (so
  // notes_touch_updated_at fires and bumps updated_at, every time - even a
  // same-parent reorder), while every renumbered sibling's row touches only
  // sort_order (so the trigger does NOT fire for them). Runs as a sequence
  // of individual statements rather than one DB transaction (no ad-hoc
  // client-side transactions over PostgREST) - acceptable for the
  // "online-only" scope, but a reorder could partially apply if the
  // connection drops mid-loop.
  async reorder(id: number, parentId: number | null, beforeId: number | null): Promise<NoteRecord> {
    await this.checkNoCycle(id, parentId);

    let siblingQuery = this.client()
      .from('notes')
      .select('id')
      .eq('notebook_id', this.notebookId)
      .neq('id', id)
      .order('sort_order', { ascending: true })
      .order('id', { ascending: true });
    siblingQuery = parentId === null ? siblingQuery.is('parent_id', null) : siblingQuery.eq('parent_id', parentId);
    const { data: siblingRows, error: siblingError } = await siblingQuery;
    if (siblingError) throw new Error(siblingError.message);

    const siblings = (siblingRows as { id: number }[]).map((row) => row.id);
    const insertAt = beforeId !== null && siblings.includes(beforeId) ? siblings.indexOf(beforeId) : siblings.length;
    siblings.splice(insertAt, 0, id);

    for (let index = 0; index < siblings.length; index++) {
      const siblingId = siblings[index];
      const patch = siblingId === id ? { parent_id: parentId, sort_order: index } : { sort_order: index };
      const { error } = await this.client().from('notes').update(patch).eq('notebook_id', this.notebookId).eq('id', siblingId);
      if (error) throw new Error(error.message);
    }

    return this.findNote(id);
  }

  async search(query: string): Promise<NoteRecord[]> {
    const notes = await this.list();
    if (!query.trim()) return notes;
    const haystack = query.toLowerCase();
    return notes.filter((note) => `${note.title} ${note.content}`.toLowerCase().includes(haystack));
  }
}

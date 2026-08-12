import type { NoteRecord } from '../services/notesService';

// Deliberately excludes nested brackets so "[[A]] and [[B]]" parses as two
// links, not one greedy match spanning both. Exported so MarkdownEditor's
// text-to-node conversion (which needs match positions, not just titles)
// can reuse the exact same pattern rather than a second, possibly-drifting
// copy.
export const WIKI_LINK_RE = /\[\[([^[\]]+)\]\]/g;

// Every [[Title]] occurrence in a note's content - used both to convert
// typed/loaded text into real wikiLink nodes (MarkdownEditor.svelte) and to
// scan every note for backlinks (below).
export function extractWikiLinkTitles(content: string): string[] {
  const titles: string[] = [];
  for (const match of content.matchAll(WIKI_LINK_RE)) {
    const title = match[1].trim();
    if (title) titles.push(title);
  }
  return titles;
}

export function normalizeWikiTitle(title: string): string {
  return title.trim().toLowerCase();
}

// Case-insensitive, trimmed exact match. First-match-wins on duplicate
// titles (this app doesn't enforce unique note titles) - a known, accepted
// limitation of resolving by title rather than a stable id.
export function resolveWikiLinkTitle(title: string, notes: NoteRecord[]): NoteRecord | null {
  const needle = normalizeWikiTitle(title);
  if (!needle) return null;
  return notes.find((n) => normalizeWikiTitle(n.title) === needle) ?? null;
}

export interface Backlink {
  noteId: number;
  title: string;
}

// Every OTHER note that references `target`'s title via [[Title]] - self-
// references excluded (a note linking to itself isn't a meaningful
// backlink). One entry per source note even if it links the target more
// than once.
export function computeBacklinks(target: NoteRecord, allNotes: NoteRecord[]): Backlink[] {
  const targetKey = normalizeWikiTitle(target.title);
  if (!targetKey) return [];
  const result: Backlink[] = [];
  for (const note of allNotes) {
    if (note.id === target.id) continue;
    const hit = extractWikiLinkTitles(note.content).some((t) => normalizeWikiTitle(t) === targetKey);
    if (hit) result.push({ noteId: note.id, title: note.title });
  }
  return result;
}

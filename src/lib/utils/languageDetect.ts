// The fixed set of languages EditorModeEditor/the language picker/the
// formatter all agree on. 'plain' means "no specific grammar" - notes that
// don't look like any of the others (including genuinely unstructured text
// like a network switch config) land here, with monospace-only styling and
// no attempted syntax highlighting or structural reformatting.
export type LanguageId = 'json' | 'javascript' | 'css' | 'html' | 'xml' | 'yaml' | 'markdown' | 'python' | 'sql' | 'shell' | 'plain';

export const LANGUAGE_OPTIONS: { id: LanguageId; label: string }[] = [
  { id: 'plain', label: 'Plain / auto' },
  { id: 'json', label: 'JSON' },
  { id: 'javascript', label: 'JavaScript / TypeScript' },
  { id: 'css', label: 'CSS' },
  { id: 'html', label: 'HTML' },
  { id: 'xml', label: 'XML' },
  { id: 'yaml', label: 'YAML' },
  { id: 'markdown', label: 'Markdown' },
  { id: 'python', label: 'Python' },
  { id: 'sql', label: 'SQL' },
  { id: 'shell', label: 'Shell' },
];

// A cheap content-sniffing heuristic, not a real language classifier -
// good enough that most notes never need the picker touched at all, and
// deliberately conservative: anything it doesn't recognize (including
// arbitrary config-style text with no standard grammar) falls back to
// 'plain' rather than guessing wrong.
export function detectLanguage(content: string): LanguageId {
  const trimmed = content.trim();
  if (!trimmed) return 'plain';

  if (trimmed.startsWith('#!')) return 'shell';

  if (/^[{[]/.test(trimmed)) {
    try {
      JSON.parse(trimmed);
      return 'json';
    } catch {
      // Falls through - could still be JS/JSON5/incomplete JSON.
    }
  }

  if (/^<\?xml/i.test(trimmed)) return 'xml';
  if (/^<!doctype html/i.test(trimmed) || /^<html[\s>]/i.test(trimmed)) return 'html';
  if (/^<[a-z][\w-]*[\s/>]/i.test(trimmed)) return 'xml';

  if (/^---\s*$/m.test(trimmed) && /^[\w-]+:\s/m.test(trimmed)) return 'yaml';

  if (/^\s*(import\s|export\s|const\s|let\s|function\s|class\s|=>)/m.test(trimmed)) return 'javascript';

  if (/^\s*(def\s|import\s|from\s.+\simport\s|class\s.+:)/m.test(trimmed)) return 'python';

  if (/^\s*(select|insert\s+into|update\s|delete\s+from|create\s+table)\b/im.test(trimmed)) return 'sql';

  return 'plain';
}

// What's ACTUALLY driving highlighting/formatting for a note: an explicit
// per-note override always wins; otherwise content-sniffing (detectLanguage
// above), since a clear signal there (this looks like JSON/HTML/etc.) is
// more trustworthy than a coarse per-note flag; only when content-sniffing
// finds nothing specific does the note's own isMarkdown flag get used as a
// fallback default - detectLanguage has no markdown-shaped heuristic at all
// (ordinary prose doesn't look like JSON/JS/YAML/etc.), so without this,
// EVERY markdown note would land on 'plain' despite FlashPad already
// knowing it's markdown. Used identically by EditorModeEditor (for
// highlighting) and App.svelte (for what the language picker displays), so
// there's exactly one place this priority order is decided.
export function resolveEffectiveLanguage(content: string, isMarkdown: boolean, override: string | null): LanguageId {
  if (override) return override as LanguageId;
  const detected = detectLanguage(content);
  if (detected !== 'plain') return detected;
  return isMarkdown ? 'markdown' : 'plain';
}

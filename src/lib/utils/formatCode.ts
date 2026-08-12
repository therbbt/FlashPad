import type { LanguageId } from './languageDetect';

interface PrettierConfig {
  parser: string;
  loadPlugins: () => Promise<unknown[]>;
}

// Only languages Prettier's standalone build handles well get real,
// language-aware formatting - everything else (python, sql, shell, plain)
// falls back to tidyWhitespace below. Each entry dynamically imports only
// the specific plugin(s) it needs, so Format's first use on a given
// language is the only time that plugin's code is fetched - normal
// startup/bundle size is unaffected.
const PRETTIER_CONFIG: Partial<Record<LanguageId, PrettierConfig>> = {
  json: {
    parser: 'json',
    loadPlugins: async () => [(await import('prettier/plugins/babel')).default, (await import('prettier/plugins/estree')).default],
  },
  javascript: {
    parser: 'babel',
    loadPlugins: async () => [(await import('prettier/plugins/babel')).default, (await import('prettier/plugins/estree')).default],
  },
  css: {
    parser: 'css',
    loadPlugins: async () => [(await import('prettier/plugins/postcss')).default],
  },
  html: {
    parser: 'html',
    loadPlugins: async () => [
      (await import('prettier/plugins/html')).default,
      (await import('prettier/plugins/postcss')).default,
      (await import('prettier/plugins/babel')).default,
      (await import('prettier/plugins/estree')).default,
    ],
  },
  yaml: {
    parser: 'yaml',
    loadPlugins: async () => [(await import('prettier/plugins/yaml')).default],
  },
  markdown: {
    parser: 'markdown',
    // Prettier's markdown plugin formats fenced code blocks (```js, ```css,
    // ...) using whichever OTHER loaded plugin matches the fence's info
    // string, automatically - it just needs those plugins present in the
    // same format() call, which is why every other plugin above is loaded
    // here too rather than only the markdown one.
    loadPlugins: async () => [
      (await import('prettier/plugins/markdown')).default,
      (await import('prettier/plugins/babel')).default,
      (await import('prettier/plugins/estree')).default,
      (await import('prettier/plugins/postcss')).default,
      (await import('prettier/plugins/html')).default,
      (await import('prettier/plugins/yaml')).default,
    ],
  },
};

// Generic, safe fallback for everything Prettier doesn't cover here
// (including arbitrary text like a switch config, where there's no
// grammar to reformat against): trims trailing whitespace per line,
// normalizes line endings, and collapses runs of blank lines - never
// reindents or reflows content, since that could silently corrupt
// something like config text where whitespace is meaningful.
export function tidyWhitespace(text: string): string {
  return text
    .replace(/\r\n?/g, '\n')
    .split('\n')
    .map((line) => line.replace(/[ \t]+$/, ''))
    .join('\n')
    .replace(/\n{3,}/g, '\n\n');
}

// Canonical per-level indent width for each recognized network-config
// dialect (see languageDetect.ts) - not a guess: matches the indentation
// actually used in real running-config output for each (Cisco IOS/Huawei
// VRP: one space per level; ADVA: two; Nokia SR OS classic CLI: four).
const NETWORK_CONFIG_INDENT_UNIT: Partial<Record<LanguageId, number>> = {
  'cisco-ios': 1,
  'huawei-vrp': 1,
  'nokia-sros': 4,
  adva: 2,
};

// These vendor CLIs have no grammar to parse against (unlike Prettier's
// languages above) and far too many context-opening keywords per
// vendor/version to enumerate safely - trying to reconstruct hierarchy from
// command semantics would be guessing. Instead this trusts the structure
// the source ALREADY encodes via its own indentation (real device output,
// or a hand-written config, is indented consistently) and just canonicalizes
// the whitespace: every distinct nonzero indent width present becomes one
// nesting level (ranked smallest to largest), then each line is re-emitted
// at `level * unit` spaces - so relative nesting is fully preserved, only
// the exact column width changes. A no-op on already-canonical input.
function reindentNetworkConfig(text: string, unit: number): string {
  const lines = tidyWhitespace(text).split('\n');

  const widths = new Set<number>();
  for (const line of lines) {
    if (!line.trim()) continue;
    const leading = line.match(/^[ \t]*/)?.[0] ?? '';
    const width = leading.replace(/\t/g, '    ').length;
    if (width > 0) widths.add(width);
  }
  const rankedWidths = [...widths].sort((a, b) => a - b);
  const depthOf = (width: number) => (width === 0 ? 0 : rankedWidths.indexOf(width) + 1);

  return lines
    .map((line) => {
      if (!line.trim()) return '';
      const leading = line.match(/^[ \t]*/)?.[0] ?? '';
      const width = leading.replace(/\t/g, '    ').length;
      return ' '.repeat(depthOf(width) * unit) + line.slice(leading.length);
    })
    .join('\n');
}

// Formats `text` for `language`. Falls back to tidyWhitespace both for
// languages with no Prettier config above AND when Prettier itself throws
// (e.g. the text is mid-edit and not currently valid syntax) - Format
// should never destroy a user's in-progress text just because it doesn't
// parse cleanly yet.
export async function formatText(text: string, language: LanguageId): Promise<string> {
  const config = PRETTIER_CONFIG[language];
  if (!config) {
    const indentUnit = NETWORK_CONFIG_INDENT_UNIT[language];
    return indentUnit != null ? reindentNetworkConfig(text, indentUnit) : tidyWhitespace(text);
  }

  try {
    const { format } = await import('prettier/standalone');
    const plugins = await config.loadPlugins();
    return await format(text, { parser: config.parser, plugins: plugins as never });
  } catch {
    return tidyWhitespace(text);
  }
}

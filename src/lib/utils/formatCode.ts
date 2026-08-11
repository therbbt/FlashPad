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
    loadPlugins: async () => [(await import('prettier/plugins/markdown')).default],
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

// Formats `text` for `language`. Falls back to tidyWhitespace both for
// languages with no Prettier config above AND when Prettier itself throws
// (e.g. the text is mid-edit and not currently valid syntax) - Format
// should never destroy a user's in-progress text just because it doesn't
// parse cleanly yet.
export async function formatText(text: string, language: LanguageId): Promise<string> {
  const config = PRETTIER_CONFIG[language];
  if (!config) return tidyWhitespace(text);

  try {
    const { format } = await import('prettier/standalone');
    const plugins = await config.loadPlugins();
    return await format(text, { parser: config.parser, plugins: plugins as never });
  } catch {
    return tidyWhitespace(text);
  }
}

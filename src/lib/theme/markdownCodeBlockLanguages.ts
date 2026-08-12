import { createLowlight } from 'lowlight';
import javascript from 'highlight.js/lib/languages/javascript';
import typescript from 'highlight.js/lib/languages/typescript';
import json from 'highlight.js/lib/languages/json';
import css from 'highlight.js/lib/languages/css';
import xml from 'highlight.js/lib/languages/xml';
import yaml from 'highlight.js/lib/languages/yaml';
import python from 'highlight.js/lib/languages/python';
import sql from 'highlight.js/lib/languages/sql';
import bash from 'highlight.js/lib/languages/bash';

// Backs CodeBlockLowlight (see MarkdownEditor.svelte) - real per-language
// token highlighting for fenced code blocks in the rich Markdown view, so
// it looks the same as Editor mode's code coloring instead of going flat
// the moment Editor mode is switched off. Same language set as Editor
// mode's markdown fenced-block support (markdownCodeLanguages.ts) minus
// html, which is just an alias of xml here.
export const markdownCodeBlockLowlight = createLowlight({ javascript, typescript, json, css, xml, yaml, python, sql, bash });

markdownCodeBlockLowlight.registerAlias({
  javascript: ['js', 'jsx', 'mjs', 'cjs'],
  typescript: ['ts', 'tsx'],
  json: ['json5', 'jsonc'],
  xml: ['html', 'htm', 'svg'],
  yaml: ['yml'],
  python: ['py'],
  bash: ['sh', 'zsh', 'shell', 'shellscript', 'console', 'git', 'terminal'],
});

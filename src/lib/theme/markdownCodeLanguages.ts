import { LanguageDescription, LanguageSupport, StreamLanguage } from '@codemirror/language';
import { parseMixed } from '@lezer/common';
import type { MarkdownExtension } from '@lezer/markdown';
import { shell } from '@codemirror/legacy-modes/mode/shell';
import { json } from '@codemirror/lang-json';
import { javascript } from '@codemirror/lang-javascript';
import { css } from '@codemirror/lang-css';
import { html } from '@codemirror/lang-html';
import { xml } from '@codemirror/lang-xml';
import { yaml } from '@codemirror/lang-yaml';
import { python } from '@codemirror/lang-python';
import { sql } from '@codemirror/lang-sql';
import { commandLineLanguage } from './commandLineHighlight';

const shellLanguage = StreamLanguage.define(shell);

// Lets fenced code blocks inside a markdown note's Editor-mode source view
// (```js, ```bash, ...) get real per-language highlighting instead of
// plain monospace text - @codemirror/lang-markdown matches each block's
// info string (the text right after the opening ```) against these
// name/alias entries. `load` is async per LanguageDescription.of's
// contract even though every one of these is already a static import -
// there's nothing to actually await.
export const MARKDOWN_CODE_LANGUAGES = [
  LanguageDescription.of({ name: 'javascript', alias: ['js', 'jsx', 'mjs', 'cjs'], load: async () => javascript() }),
  LanguageDescription.of({ name: 'typescript', alias: ['ts', 'tsx'], load: async () => javascript({ typescript: true }) }),
  LanguageDescription.of({ name: 'json', alias: ['json5', 'jsonc'], load: async () => json() }),
  LanguageDescription.of({ name: 'css', load: async () => css() }),
  LanguageDescription.of({ name: 'html', alias: ['htm'], load: async () => html() }),
  LanguageDescription.of({ name: 'xml', alias: ['svg'], load: async () => xml() }),
  LanguageDescription.of({ name: 'yaml', alias: ['yml'], load: async () => yaml() }),
  LanguageDescription.of({ name: 'python', alias: ['py'], load: async () => python() }),
  LanguageDescription.of({ name: 'sql', load: async () => sql() }),
  // Covers Linux/git/general command-line snippets - the one case the
  // fenced-block info string can't distinguish further (there's no separate
  // "git" grammar; a `git clone ...` line is just a shell command).
  LanguageDescription.of({
    name: 'shell',
    alias: ['bash', 'sh', 'zsh', 'shellscript', 'console', 'git', 'terminal'],
    load: async () => new LanguageSupport(shellLanguage),
  }),
];

// Inline (single-backtick) code spans have no info-string/language tag at
// all in CommonMark - there's no way to tell "`git clone ...`" apart from
// "`myVariable`" - so unlike the fenced-block languages above, this always
// treats inline code as a command line. A deliberate tradeoff (confirmed
// with the user): commands read correctly, at the cost of non-command
// inline code getting the same coloring since it can't be told apart.
// Uses the generic commandLineLanguage (see commandLineHighlight.ts)
// rather than the real shell grammar above - verified that grammar only
// colors a small fixed whitelist of known bash builtins/keywords, so
// something like "git tag v0.0.0" or "npm install foo" got no color at
// all. @codemirror/lang-markdown's own `codeLanguages` option only nests
// into FencedCode, so this is wired up by hand via @lezer/markdown's mixed-
// parsing extension point instead, nesting into the range between
// InlineCode's two CodeMark delimiters (its backticks - there's no separate
// "just the text" child node for inline code the way FencedCode has
// CodeText).
export const INLINE_CODE_SHELL_EXTENSION: MarkdownExtension = {
  wrap: parseMixed((node) => {
    if (node.type.name !== 'InlineCode') return null;
    const { firstChild, lastChild } = node.node;
    if (!firstChild || !lastChild || firstChild === lastChild) return null;
    const from = firstChild.to;
    const to = lastChild.from;
    if (to <= from) return null;
    return { parser: commandLineLanguage.parser, overlay: [{ from, to }] };
  }),
};

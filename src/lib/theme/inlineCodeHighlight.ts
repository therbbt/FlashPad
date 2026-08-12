import { Extension } from '@tiptap/core';
import { Plugin, PluginKey } from '@tiptap/pm/state';
import { Decoration, DecorationSet } from '@tiptap/pm/view';
import type { Node as ProseMirrorNode } from '@tiptap/pm/model';
import { tokenizeCommandLine } from './commandLineHighlight';

// Inline (single-backtick) code has no fenced-block language tag to key
// off, same limitation as Editor mode's inline code (see
// markdownCodeLanguages.ts's INLINE_CODE_SHELL_EXTENSION) - always
// highlighted as a generic command line (not a real shell grammar - see
// commandLineHighlight.ts for why), the confirmed tradeoff there too:
// commands read correctly, non-command inline code (variable names, etc.)
// gets the same coloring since it can't be told apart.
//
// CodeBlockLowlight (see MarkdownEditor.svelte) only decorates the
// `codeBlock` NODE type - inline code is a `code` MARK on ordinary text,
// which it never touches - so this is a small hand-rolled ProseMirror
// plugin producing decorations for text runs carrying the `code` mark,
// reusing the same .hljs-* classes/colors the code-block CSS already
// defines (keyword/attribute/string/number) so both look identical.
const TAG_TO_CLASS: Record<string, string> = {
  keyword: 'hljs-keyword',
  attribute: 'hljs-attr',
  string: 'hljs-string',
  number: 'hljs-number',
};

function buildDecorations(doc: ProseMirrorNode): DecorationSet {
  const decorations: Decoration[] = [];
  doc.descendants((node, pos) => {
    if (!node.isText || !node.text) return;
    if (!node.marks.some((mark) => mark.type.name === 'code')) return;
    let from = pos;
    for (const { text, tag } of tokenizeCommandLine(node.text)) {
      const to = from + text.length;
      const className = tag ? TAG_TO_CLASS[tag] : undefined;
      if (className) decorations.push(Decoration.inline(from, to, { class: className }));
      from = to;
    }
  });
  return DecorationSet.create(doc, decorations);
}

export const InlineCodeHighlight = Extension.create({
  name: 'inlineCodeHighlight',
  addProseMirrorPlugins() {
    const key = new PluginKey('inlineCodeHighlight');
    return [
      new Plugin({
        key,
        state: {
          init: (_, { doc }) => buildDecorations(doc),
          apply: (tr, old) => (tr.docChanged ? buildDecorations(tr.doc) : old),
        },
        props: {
          decorations(state) {
            return key.getState(state);
          },
        },
      }),
    ];
  },
});

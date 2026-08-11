import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import { EditorView } from '@codemirror/view';
import type { Extension } from '@codemirror/state';
import { tags as t } from '@lezer/highlight';

// Fixed syntax-token colors for editor mode's "real code editor" look -
// deliberately separate from the app's multi-palette theme system
// (palettes.ts), which only defines UI chrome colors (bg/panel/text/etc),
// not per-token syntax colors. Picked per light/dark mode rather than per
// individual palette, matching how a palette's own `mode` field already
// only has two values - the editor chrome below still rides whichever
// palette is actually active via CSS custom properties.
const darkHighlightStyle = HighlightStyle.define([
  { tag: t.keyword, color: '#c792ea' },
  { tag: [t.name, t.deleted, t.character, t.macroName], color: '#f07178' },
  { tag: t.propertyName, color: '#82aaff' },
  { tag: [t.function(t.variableName), t.labelName], color: '#82aaff' },
  { tag: [t.color, t.constant(t.name), t.standard(t.name)], color: '#f78c6c' },
  { tag: [t.definition(t.name), t.separator], color: '#eeffff' },
  { tag: [t.typeName, t.className, t.number, t.changed, t.annotation, t.modifier, t.self, t.namespace], color: '#f78c6c' },
  { tag: [t.operator, t.operatorKeyword, t.url, t.escape, t.regexp, t.link, t.special(t.string)], color: '#89ddff' },
  { tag: [t.meta, t.comment], color: '#7d84a8', fontStyle: 'italic' },
  { tag: t.strong, fontWeight: 'bold' },
  { tag: t.emphasis, fontStyle: 'italic' },
  { tag: t.strikethrough, textDecoration: 'line-through' },
  { tag: t.link, color: '#82aaff', textDecoration: 'underline' },
  { tag: t.heading, fontWeight: 'bold', color: '#c792ea' },
  { tag: [t.atom, t.bool, t.special(t.variableName)], color: '#f78c6c' },
  { tag: [t.processingInstruction, t.string, t.inserted], color: '#c3e88d' },
  { tag: t.invalid, color: '#ff5370' },
]);

const lightHighlightStyle = HighlightStyle.define([
  { tag: t.keyword, color: '#8959a8' },
  { tag: [t.name, t.deleted, t.character, t.macroName], color: '#c82829' },
  { tag: t.propertyName, color: '#4271ae' },
  { tag: [t.function(t.variableName), t.labelName], color: '#4271ae' },
  { tag: [t.color, t.constant(t.name), t.standard(t.name)], color: '#b5651d' },
  { tag: [t.definition(t.name), t.separator], color: '#4d4d4c' },
  { tag: [t.typeName, t.className, t.number, t.changed, t.annotation, t.modifier, t.self, t.namespace], color: '#b5651d' },
  { tag: [t.operator, t.operatorKeyword, t.url, t.escape, t.regexp, t.link, t.special(t.string)], color: '#3e999f' },
  { tag: [t.meta, t.comment], color: '#8e908c', fontStyle: 'italic' },
  { tag: t.strong, fontWeight: 'bold' },
  { tag: t.emphasis, fontStyle: 'italic' },
  { tag: t.strikethrough, textDecoration: 'line-through' },
  { tag: t.link, color: '#4271ae', textDecoration: 'underline' },
  { tag: t.heading, fontWeight: 'bold', color: '#8959a8' },
  { tag: [t.atom, t.bool, t.special(t.variableName)], color: '#b5651d' },
  { tag: [t.processingInstruction, t.string, t.inserted], color: '#718c00' },
  { tag: t.invalid, color: '#c82829' },
]);

// Editor chrome (background/gutters/selection/search panel) rides the
// app's own CSS custom properties so it fits whichever palette is
// currently active - only the syntax token colors above are hardcoded per
// light/dark mode.
const chromeTheme = EditorView.theme({
  '&': { color: 'inherit', backgroundColor: 'transparent', height: '100%' },
  '&.cm-focused': { outline: 'none' },
  '.cm-content': { fontFamily: "ui-monospace, 'SF Mono', Menlo, Consolas, monospace", padding: '1rem 1.1rem', caretColor: 'currentColor' },
  '.cm-scroller': { fontFamily: 'inherit', lineHeight: '1.55' },
  '.cm-line': { padding: '0' },
  '.cm-gutters': { backgroundColor: 'transparent', color: 'var(--muted)', border: 'none', borderRight: '1px solid var(--border)' },
  '.cm-lineNumbers .cm-gutterElement': { padding: '0 0.6rem 0 0.7rem', fontSize: '0.75em' },
  '.cm-activeLine': { backgroundColor: 'var(--panel-2)' },
  '.cm-activeLineGutter': { backgroundColor: 'var(--panel-2)' },
  '.cm-selectionBackground, &.cm-focused .cm-selectionBackground': { backgroundColor: 'var(--accent-soft, rgba(91, 155, 213, 0.35)) !important' },
  '.cm-cursor': { borderLeftColor: 'currentColor !important' },
  '.cm-searchMatch': { backgroundColor: 'rgba(255, 210, 0, 0.25)' },
  '.cm-searchMatch.cm-searchMatch-selected': { backgroundColor: 'rgba(255, 165, 0, 0.45)' },

  // Rebuilds @codemirror/search's default panel look from its actual DOM
  // shape (see node_modules/@codemirror/search's SearchPanel: a single
  // .cm-search div holding a find <input>, next/prev/all <button>s, three
  // checkbox <label>s, a <br>, then (when the doc is editable) a replace
  // <input> and two more buttons, then a name="close" button) rather than
  // overriding its bundled baseTheme piecemeal - that default look is what
  // read as out of place against the rest of the app's UI.
  '.cm-panels': { backgroundColor: 'var(--panel)', color: 'var(--text)' },
  '.cm-panels.cm-panels-bottom': { borderTop: '1px solid var(--border)' },
  '.cm-panel.cm-search': {
    display: 'flex',
    flexWrap: 'wrap',
    alignItems: 'center',
    gap: '0.4rem',
    padding: '0.5rem 2.2rem 0.5rem 0.6rem',
  },
  '.cm-search br': { flexBasis: '100%', height: '0' },
  '.cm-search .cm-textfield': {
    background: 'var(--panel-2)',
    color: 'var(--text)',
    border: '1px solid var(--border)',
    borderRadius: '0.3rem',
    padding: '0.25rem 0.5rem',
    fontSize: '0.78rem',
    margin: '0',
    minWidth: '9rem',
  },
  '.cm-search .cm-textfield:focus': { outline: '1px solid var(--accent-soft, var(--accent))' },
  '.cm-search .cm-button': {
    background: 'var(--panel-2)',
    color: 'var(--text)',
    border: '1px solid var(--border)',
    borderRadius: '0.3rem',
    padding: '0.25rem 0.55rem',
    fontSize: '0.75rem',
    margin: '0',
    backgroundImage: 'none',
  },
  '.cm-search .cm-button:hover': { background: 'var(--accent-soft, var(--panel-2))' },
  '.cm-search label': {
    display: 'inline-flex',
    alignItems: 'center',
    gap: '0.3rem',
    margin: '0',
    fontSize: '0.72rem',
    color: 'var(--muted)',
    whiteSpace: 'nowrap',
  },
  '.cm-search input[type=checkbox]': { margin: '0', accentColor: 'var(--accent)' },
  '.cm-search button[name=close]': {
    position: 'absolute',
    top: '0.4rem',
    right: '0.5rem',
    width: '1.4rem',
    height: '1.4rem',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    background: 'transparent',
    color: 'var(--muted)',
    border: 'none',
    borderRadius: '0.3rem',
    font: 'inherit',
    fontSize: '0.9rem',
    padding: '0',
    margin: '0',
  },
  '.cm-search button[name=close]:hover': { background: 'var(--panel-2)', color: 'var(--text)' },
});

export function codeEditorTheme(mode: 'light' | 'dark'): Extension {
  return [chromeTheme, syntaxHighlighting(mode === 'dark' ? darkHighlightStyle : lightHighlightStyle)];
}

<script lang="ts">
  // Mirrors MarkdownEditor.svelte's interface (content/noteId/onUpdate/
  // placeholder/editable) so App.svelte's dual-editor dispatch pattern
  // (isMarkdownActive ? markdownEditorRef : plainEditorRef) needs no special
  // casing beyond which ref it calls.
  //
  // Built on CodeMirror 6 rather than a raw <textarea> specifically so vim
  // mode (@replit/codemirror-vim) can be layered on top - CM6's own
  // history()/lineNumbers() extensions also replace what used to be a
  // hand-rolled undo stack and a manually scroll-synced <pre> gutter, so
  // this is a net simplification, not just an addition.
  import { onMount, onDestroy } from 'svelte';
  import { EditorState, Compartment } from '@codemirror/state';
  import { EditorView, keymap, lineNumbers, drawSelection, placeholder as placeholderExt } from '@codemirror/view';
  import { defaultKeymap, historyKeymap, history } from '@codemirror/commands';
  import { vim, getCM } from '@replit/codemirror-vim';
  import { vimModeIndicator } from '../stores/vimModeIndicator';
  import { toLfNewlines } from '../utils/clipboard';
  import { detectLanguage, type LanguageId } from '../utils/languageDetect';
  import { formatText } from '../utils/formatCode';

  export let content: string;
  export let noteId: number;
  export let onUpdate: (text: string) => void;
  export let placeholder = '';
  export let editable = true;
  export let showLineNumbers = false;
  export let vimMode = false;
  // The note's persisted editor-mode language override (see
  // EditorModeEditor.svelte) - Format uses this (falling back to
  // auto-detection) even here in the plain view, so a note's language
  // choice applies consistently no matter which view you format it from.
  export let language: string | null = null;

  let container: HTMLDivElement;
  let view: EditorView | undefined;
  let lastNoteId: number | undefined;
  let unsubscribeVimModeChange: (() => void) | undefined;

  const vimCompartment = new Compartment();
  const lineDisplayCompartment = new Compartment();
  const editableCompartment = new Compartment();

  // Plain-text notes should never pick up rich formatting from the
  // clipboard (bold/colors/fonts from a pasted webpage, Word doc, etc.) -
  // force the text/plain flavor regardless of how Ctrl+V or the OS's own
  // "Paste" action populated the clipboard. Markdown notes keep the
  // editor's normal paste handling, which is expected to preserve
  // formatting. This is orthogonal to vim's own p/P (those operate on
  // vim's internal register, not the OS clipboard, and never fire this
  // DOM "paste" event).
  //
  // Even the text/plain flavor isn't clean, though: when copying from a
  // rendered webpage, browsers generate that plain-text fallback from the
  // page's DOM structure, which bakes in each source element's indentation
  // as literal leading spaces/tabs on every line - strip those per line so
  // pasted lines start flush left, matching what plain notes expect.
  //
  // CRLF must be folded to LF *before* any of that, and before the inserted
  // text's length is used below: CM6 splits the inserted string on
  // /\r\n?|\n/ and stores lines without the separator, so every \r vanishes
  // from the document while still counting towards `text.length`. The
  // resulting cursor anchor then points past the end of the document, which
  // makes the whole dispatch throw - and since the paste event was already
  // preventDefault-ed, a multi-line paste from a Windows app (which puts
  // CRLF on the clipboard) would silently do nothing at all.
  const cleanPastedText = (raw: string) =>
    toLfNewlines(raw)
      .split('\n')
      .map((line) => line.replace(/^[ \t]+/, ''))
      .join('\n');

  // Line numbers are per logical line, matching how editors typically
  // define "line numbers". Wrapping is disabled while the gutter shows so
  // each logical line renders as exactly one row and stays aligned with
  // its number - CM6 doesn't wrap by default, so the non-gutter case has
  // to opt back into wrapping instead.
  const lineDisplayExtensions = (show: boolean) => (show ? [lineNumbers()] : [EditorView.lineWrapping]);

  const editableExtensions = (isEditable: boolean) => [
    EditorView.editable.of(isEditable),
    EditorState.readOnly.of(!isEditable),
  ];

  const formatVimMode = (mode: string, subMode?: string) => {
    if (mode === 'visual') {
      if (subMode === 'linewise') return 'V-LINE';
      if (subMode === 'blockwise') return 'V-BLOCK';
      return 'VISUAL';
    }
    return mode.toUpperCase();
  };

  const applyVimMode = (enabled: boolean) => {
    if (!view) return;
    view.dispatch({ effects: vimCompartment.reconfigure(enabled ? [vim()] : []) });
    unsubscribeVimModeChange?.();
    unsubscribeVimModeChange = undefined;
    vimModeIndicator.set(null);
    if (!enabled) return;
    const cm = getCM(view);
    if (!cm) return;
    const handleModeChange = (e: { mode: string; subMode?: string }) => {
      vimModeIndicator.set(formatVimMode(e.mode, e.subMode));
    };
    cm.on('vim-mode-change', handleModeChange);
    unsubscribeVimModeChange = () => cm.off('vim-mode-change', handleModeChange);
    vimModeIndicator.set('NORMAL');
  };

  // No ex command-line: FlashPad already auto-saves, so :w/:q-style
  // commands have nothing meaningful to do, and the command bar itself is
  // more UI than a single-pane note app needs. `Vim.unmap(':', ...)`
  // doesn't actually stop the dialog (colon is wired in below the
  // remappable keymap layer), so this intercepts the keydown itself
  // instead - in the capture phase on view.dom, an ANCESTOR of the actual
  // focused element (view.contentDOM), so it runs before vim's own
  // handler even sees the event, and stopPropagation() here keeps it from
  // ever reaching contentDOM at all. Skipped while in insert mode so a
  // literal ':' still types normally (e.g. writing a time like "9:30").
  // `/` search is a separate, still-active binding, unaffected by this.
  const interceptColonWhileVimActive = (event: KeyboardEvent) => {
    if (!vimMode || event.key !== ':' || !view) return;
    if (getCM(view)?.state.vim?.insertMode) return;
    event.preventDefault();
    event.stopPropagation();
  };

  // Vim's own Escape (leave insert/visual mode) must not bubble to
  // App.svelte's global window keydown handler, which hides the whole app
  // window on a bare Escape when no dialog is open. Attached directly on
  // the editor's DOM so it runs before ancestor bubble-phase listeners.
  // Only active while vim mode is on - Escape keeps bubbling (today's
  // behavior) when it's off.
  const stopEscapeWhileVimActive = (event: KeyboardEvent) => {
    if (vimMode && event.key === 'Escape') event.stopPropagation();
  };

  onMount(() => {
    view = new EditorView({
      parent: container,
      state: EditorState.create({
        doc: content,
        extensions: [
          // vim must come before other keymaps so it gets first crack at keys.
          vimCompartment.of(vimMode ? [vim()] : []),
          history(),
          keymap.of([...defaultKeymap, ...historyKeymap]),
          drawSelection(),
          lineDisplayCompartment.of(lineDisplayExtensions(showLineNumbers)),
          editableCompartment.of(editableExtensions(editable)),
          ...(placeholder ? [placeholderExt(placeholder)] : []),
          EditorView.updateListener.of((update) => {
            if (update.docChanged) onUpdate(update.state.doc.toString());
          }),
          EditorView.domEventHandlers({
            paste: (event, editorView) => {
              event.preventDefault();
              const raw = event.clipboardData?.getData('text/plain') ?? '';
              const text = cleanPastedText(raw);
              const { from, to } = editorView.state.selection.main;
              editorView.dispatch({
                changes: { from, to, insert: text },
                selection: { anchor: from + text.length },
                scrollIntoView: true,
              });
              return true;
            },
          }),
          EditorView.theme({
            '&': { color: 'inherit', backgroundColor: 'transparent', height: '100%' },
            '&.cm-focused': { outline: 'none' },
            '.cm-content': { fontFamily: 'inherit', padding: '1rem 1.1rem', caretColor: 'currentColor' },
            '.cm-scroller': { fontFamily: 'inherit', lineHeight: '1.55' },
            '.cm-line': { padding: '0' },
            '.cm-gutters': { backgroundColor: 'transparent', color: 'var(--muted)', border: 'none', borderRight: '1px solid var(--border)' },
            '.cm-lineNumbers .cm-gutterElement': { padding: '0 0.6rem 0 0.7rem', fontSize: '0.75em' },
            '.cm-placeholder': { color: 'var(--muted)', opacity: '1' },
            // drawSelection()'s own thin caret (shown whenever vim mode is
            // off, or in vim's insert mode) hardcodes a black border by
            // default (@codemirror/view's base theme: ".cm-cursor { border-
            // left: 1.2px solid black }") - it only lightens that for CM6's
            // own notion of a "dark" theme, which isn't set here since
            // FlashPad has its own multi-palette system rather than a
            // binary light/dark flag. currentColor keeps it in sync with
            // whichever palette is active; !important for the same
            // cascade-ordering reason as the fat-cursor override below.
            '.cm-cursor': { borderLeftColor: 'currentColor !important' },
            // @replit/codemirror-vim's block ("fat") cursor - shown in normal/
            // visual mode in place of the usual thin caret - hardcodes a
            // salmon-pink background (#ff9696) with a plain `background:`
            // declaration (no !important), regardless of app theme. Its own
            // theme is registered with Prec.highest, so a plain override
            // here isn't guaranteed to win the CSS cascade - !important
            // makes it win outright instead of depending on injection order.
            // Color matches the "search all databases" toggle's active
            // teal (Footer.svelte's .search-scope-btn.active) - that color
            // isn't part of the palette system (no CSS variable for it,
            // unlike --accent/--md-color), so it's the same literal here.
            '.cm-fat-cursor': { background: '#4dd0c8 !important' },
            '&:not(.cm-focused) .cm-fat-cursor': {
              background: 'none !important',
              outline: 'solid 1px #4dd0c8 !important',
              color: 'transparent !important',
            },
          }),
        ],
      }),
    });
    view.dom.addEventListener('keydown', stopEscapeWhileVimActive);
    view.dom.addEventListener('keydown', interceptColonWhileVimActive, true);
    lastNoteId = noteId;
    // `view` becoming truthy re-triggers the `applyVimMode` reactive
    // statement below, which performs the initial vim-mode setup - no need
    // to call it here too.
  });

  onDestroy(() => {
    unsubscribeVimModeChange?.();
    vimModeIndicator.set(null);
    view?.dom.removeEventListener('keydown', interceptColonWhileVimActive, true);
    view?.destroy();
  });

  // Resyncs from the parent's content only when the *selected note*
  // changes - not on every keystroke, matching MarkdownEditor's
  // noteId-gated setContent.
  $: if (view && noteId !== lastNoteId) {
    view.dispatch({
      changes: { from: 0, to: view.state.doc.length, insert: content },
      selection: { anchor: 0 },
    });
    lastNoteId = noteId;
  }

  $: if (view) view.dispatch({ effects: lineDisplayCompartment.reconfigure(lineDisplayExtensions(showLineNumbers)) });
  $: if (view) view.dispatch({ effects: editableCompartment.reconfigure(editableExtensions(editable)) });
  $: if (view) applyVimMode(vimMode);

  export function focus() {
    view?.focus();
  }

  export function insertAtCursor(raw: string) {
    if (!view) return;
    // Same CRLF-vs-anchor hazard as cleanPastedText above.
    const text = toLfNewlines(raw);
    const { from, to } = view.state.selection.main;
    view.dispatch({
      changes: { from, to, insert: text },
      selection: { anchor: from + text.length },
      scrollIntoView: true,
    });
    view.focus();
  }

  // Formats just the current selection if there is one, otherwise the
  // whole note - see formatCode.ts for the language-aware/tidy-up split.
  export async function format(): Promise<void> {
    if (!view) return;
    const { from, to } = view.state.selection.main;
    const hasSelection = from !== to;
    const range = hasSelection ? { from, to } : { from: 0, to: view.state.doc.length };
    const source = view.state.doc.sliceString(range.from, range.to);
    const effectiveLanguage = (language as LanguageId | null) ?? detectLanguage(content);
    const formatted = await formatText(source, effectiveLanguage);
    if (formatted === source) return;
    view.dispatch({ changes: { from: range.from, to: range.to, insert: formatted } });
  }
</script>

<div class="plain-editor-wrap" bind:this={container}></div>

<style>
  .plain-editor-wrap {
    flex: 1;
    display: flex;
    min-height: 0;
  }

  .plain-editor-wrap :global(.cm-editor) {
    flex: 1;
    width: 100%;
    min-width: 0;
  }
</style>

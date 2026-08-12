<script lang="ts">
  // Global code-editor view (see settingsService.ts's editorMode toggle) -
  // takes over from PlainTextEditor/MarkdownEditor for every note while
  // enabled. Built the same way PlainTextEditor.svelte is (same
  // content/noteId/onUpdate/placeholder/editable prop contract, same
  // Compartment-based extension-swapping pattern, same exported
  // focus()/insertAtCursor()), plus language support, search/replace, a
  // code-editor color scheme, and Format.
  import { onMount, onDestroy } from 'svelte';
  import { EditorState, Compartment } from '@codemirror/state';
  import { EditorView, keymap, lineNumbers, drawSelection, placeholder as placeholderExt } from '@codemirror/view';
  import { defaultKeymap, historyKeymap, indentWithTab, history } from '@codemirror/commands';
  import { search, searchKeymap, openSearchPanel, closeSearchPanel, searchPanelOpen } from '@codemirror/search';
  import type { LanguageSupport } from '@codemirror/language';
  import { vim, getCM } from '@replit/codemirror-vim';
  import { json } from '@codemirror/lang-json';
  import { javascript } from '@codemirror/lang-javascript';
  import { css } from '@codemirror/lang-css';
  import { html } from '@codemirror/lang-html';
  import { xml } from '@codemirror/lang-xml';
  import { yaml } from '@codemirror/lang-yaml';
  import { markdown } from '@codemirror/lang-markdown';
  import { python } from '@codemirror/lang-python';
  import { sql } from '@codemirror/lang-sql';
  import { codeEditorTheme } from '../theme/codeEditorTheme';
  import { NETWORK_CONFIG_LANGUAGE_SUPPORT } from '../theme/networkConfigLanguage';
  import { MARKDOWN_CODE_LANGUAGES, INLINE_CODE_SHELL_EXTENSION } from '../theme/markdownCodeLanguages';
  import { resolveEffectiveLanguage, type LanguageId } from '../utils/languageDetect';
  import { formatText } from '../utils/formatCode';
  import { vimModeIndicator } from '../stores/vimModeIndicator';
  import { toLfNewlines } from '../utils/clipboard';

  export let content: string;
  export let noteId: number;
  export let onUpdate: (text: string) => void;
  export let placeholder = '';
  export let editable = true;
  export let showLineNumbers = false;
  export let vimMode = false;
  export let themeMode: 'light' | 'dark' = 'dark';
  // The note's PERSISTED language override - null means "auto-detect from
  // content" (see languageDetect.ts). The picker that sets this lives in
  // NoteInfoPopover.svelte now (App.svelte owns onLanguageChange there) -
  // this component only needs the current value, to know what to
  // highlight/format against.
  export let language: string | null = null;
  // The note's own markdown flag (same one the Plain/Markdown toggle
  // uses) - a fallback default (see resolveEffectiveLanguage) for when
  // content-sniffing finds nothing specific, since detectLanguage() has no
  // markdown-shaped heuristic at all (prose doesn't look like JSON/JS/
  // YAML/etc.) despite FlashPad already knowing it's markdown. A clear
  // content signal (this looks like HTML/JSON/etc.) still wins over this,
  // even in a markdown-flagged note.
  export let isMarkdown = false;

  let container: HTMLDivElement;
  let view: EditorView | undefined;
  let lastNoteId: number | undefined;
  let unsubscribeVimModeChange: (() => void) | undefined;

  const vimCompartment = new Compartment();
  const lineDisplayCompartment = new Compartment();
  const editableCompartment = new Compartment();
  const languageCompartment = new Compartment();
  const themeCompartment = new Compartment();

  const LANGUAGE_SUPPORT: Partial<Record<LanguageId, () => LanguageSupport>> = {
    json,
    javascript,
    css,
    html,
    xml,
    yaml,
    // Overrides the bare `markdown` import - codeLanguages lets fenced code
    // blocks (```js, ```bash, ...) delegate to their own real grammar
    // instead of rendering as plain unhighlighted text.
    markdown: () => markdown({ codeLanguages: MARKDOWN_CODE_LANGUAGES, extensions: [INLINE_CODE_SHELL_EXTENSION] }),
    python,
    sql,
    ...NETWORK_CONFIG_LANGUAGE_SUPPORT,
  };

  // 'shell' and 'plain' (and anything unrecognized) get no LanguageSupport
  // at all - monospace text with no syntax highlighting, rather than
  // guessing wrong with a mismatched grammar.
  const languageExtension = (id: LanguageId) => {
    const support = LANGUAGE_SUPPORT[id];
    return support ? [support()] : [];
  };

  // What's actually driving highlighting/formatting right now: the note's
  // pinned language if it has one, otherwise a live guess from the current
  // content - recomputed reactively, so the guess can shift as you type
  // until you pin one explicitly via the picker.
  $: effectiveLanguage = resolveEffectiveLanguage(content, isMarkdown, language);
  $: if (view) view.dispatch({ effects: languageCompartment.reconfigure(languageExtension(effectiveLanguage)) });
  $: if (view) view.dispatch({ effects: themeCompartment.reconfigure(codeEditorTheme(themeMode)) });

  // Same clipboard handling as PlainTextEditor - editor-mode notes are code/
  // config text, never rich formatting from the clipboard.
  const cleanPastedText = (raw: string) =>
    toLfNewlines(raw)
      .split('\n')
      .map((line) => line.replace(/^[ \t]+/, ''))
      .join('\n');

  // Mod-F toggles rather than only ever opening - @codemirror/search's own
  // default Mod-f binding (openSearchPanel) just refocuses the search field
  // if the panel's already open, with no built-in way to close it from the
  // same key. Swapped into a copy of searchKeymap below, and reused by the
  // toolbar's Search button (openSearch()) so both behave identically.
  const toggleSearch = (targetView: EditorView) => {
    if (searchPanelOpen(targetView.state)) {
      closeSearchPanel(targetView);
    } else {
      openSearchPanel(targetView);
    }
    return true;
  };
  const editorSearchKeymap = searchKeymap.map((binding) => (binding.key === 'Mod-f' ? { ...binding, run: toggleSearch } : binding));

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

  const interceptColonWhileVimActive = (event: KeyboardEvent) => {
    if (!vimMode || event.key !== ':' || !view) return;
    if (getCM(view)?.state.vim?.insertMode) return;
    event.preventDefault();
    event.stopPropagation();
  };

  const stopEscapeWhileVimActive = (event: KeyboardEvent) => {
    if (vimMode && event.key === 'Escape') event.stopPropagation();
  };

  onMount(() => {
    view = new EditorView({
      parent: container,
      state: EditorState.create({
        doc: content,
        extensions: [
          vimCompartment.of(vimMode ? [vim()] : []),
          history(),
          // Docked at the bottom (closer to the app's own footer search box)
          // rather than CodeMirror's default top-of-editor placement, which
          // pushed note text down and felt disconnected from the rest of
          // the app's UI.
          search({ top: false }),
          // CodeMirror deliberately leaves Tab unbound by default (so it
          // falls through to the browser's normal focus-navigation) unless
          // indentWithTab is added explicitly - in a real code/config
          // editor, Tab indenting the text is the expected behavior
          // (matches VSCode etc.), so it's opted back in here.
          // Alt-c bound here (rather than left to bubble up to App.svelte's
          // window keydown handler) for the same reason Ctrl+F is handled
          // entirely inside CodeMirror's own keymap already - Alt-key
          // combos aren't reliably delivered to a window-level listener
          // from inside this editable surface in the actual Tauri/
          // WebKitGTK build, only to a keymap registered on the editor
          // itself.
          keymap.of([{ key: 'Alt-c', run: () => { insertInlineCode(); return true; }, preventDefault: true }, ...defaultKeymap, ...historyKeymap, ...editorSearchKeymap, indentWithTab]),
          drawSelection(),
          lineDisplayCompartment.of(lineDisplayExtensions(showLineNumbers)),
          editableCompartment.of(editableExtensions(editable)),
          languageCompartment.of(languageExtension(effectiveLanguage)),
          themeCompartment.of(codeEditorTheme(themeMode)),
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
        ],
      }),
    });
    view.dom.addEventListener('keydown', stopEscapeWhileVimActive);
    view.dom.addEventListener('keydown', interceptColonWhileVimActive, true);
    lastNoteId = noteId;
  });

  onDestroy(() => {
    unsubscribeVimModeChange?.();
    vimModeIndicator.set(null);
    view?.dom.removeEventListener('keydown', interceptColonWhileVimActive, true);
    view?.destroy();
  });

  // Resyncs from the parent's content only when the *selected note*
  // changes - not on every keystroke, matching PlainTextEditor.
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
    const text = toLfNewlines(raw);
    const { from, to } = view.state.selection.main;
    view.dispatch({
      changes: { from, to, insert: text },
      selection: { anchor: from + text.length },
      scrollIntoView: true,
    });
    view.focus();
  }

  // Alt+C - wraps the current selection in backticks (inline code), or
  // inserts an empty pair with the cursor placed between them if nothing
  // is selected, so typing can continue immediately.
  export function insertInlineCode() {
    if (!view || !editable) return;
    const { from, to } = view.state.selection.main;
    if (from === to) {
      view.dispatch({
        changes: { from, insert: '``' },
        selection: { anchor: from + 1 },
        scrollIntoView: true,
      });
    } else {
      const text = view.state.sliceDoc(from, to);
      view.dispatch({
        changes: { from, to, insert: `\`${text}\`` },
        selection: { anchor: from + 1, head: from + 1 + text.length },
        scrollIntoView: true,
      });
    }
    view.focus();
  }

  export function openSearch() {
    if (view) toggleSearch(view);
  }

  // Used by the right-click menu (App.svelte's openEditorMenu) to decide
  // whether Copy/Cut should act on the OS clipboard (a real selection) or
  // fall back to FlashPad's own note-level tree clipboard (nothing selected).
  export function getSelectedText(): string {
    if (!view) return '';
    const { from, to } = view.state.selection.main;
    return view.state.sliceDoc(from, to);
  }

  // Deletes the current selection and returns the text that was removed
  // ('' if there was no selection) - the caller is responsible for putting
  // the result on the OS clipboard.
  export function cutSelection(): string {
    if (!view) return '';
    const { from, to } = view.state.selection.main;
    if (from === to) return '';
    const text = view.state.sliceDoc(from, to);
    view.dispatch({ changes: { from, to, insert: '' } });
    return text;
  }

  // Formats just the current selection if there is one, otherwise the
  // whole note - see formatCode.ts for the language-aware/tidy-up split.
  export async function format(): Promise<void> {
    if (!view) return;
    const { from, to } = view.state.selection.main;
    const hasSelection = from !== to;
    const range = hasSelection ? { from, to } : { from: 0, to: view.state.doc.length };
    const source = view.state.doc.sliceString(range.from, range.to);
    const formatted = await formatText(source, effectiveLanguage);
    if (formatted === source) return;
    view.dispatch({ changes: { from: range.from, to: range.to, insert: formatted } });
  }
</script>

<div class="editor-mode-wrap">
  <div class="cm-container" bind:this={container}></div>
</div>

<style>
  .editor-mode-wrap {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .cm-container {
    flex: 1;
    display: flex;
    min-height: 0;
  }

  .cm-container :global(.cm-editor) {
    flex: 1;
    width: 100%;
    min-width: 0;
  }
</style>

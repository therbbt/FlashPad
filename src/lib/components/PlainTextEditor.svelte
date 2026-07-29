<script lang="ts">
  // Mirrors MarkdownEditor.svelte's interface (content/noteId/onUpdate/
  // placeholder/editable) so App.svelte's dual-editor dispatch pattern
  // (isMarkdownActive ? markdownEditorRef : plainEditorRef) needs no special
  // casing beyond which ref it calls.
  export let content: string;
  export let noteId: number;
  export let onUpdate: (text: string) => void;
  export let placeholder = '';
  export let editable = true;
  export let showLineNumbers = false;

  let textarea: HTMLTextAreaElement;
  let gutterEl: HTMLPreElement | undefined;
  let lastNoteId: number | undefined;
  let value = content;

  // Own undo/redo stack, independent of the browser's native textarea undo -
  // Markdown notes already have reliable undo via Tiptap/ProseMirror's
  // history extension, but the native undo manager for a bound <textarea>
  // isn't dependable across platforms (WebKitGTK on Linux in particular).
  let plainUndoStack: { value: string; start: number; end: number }[] = [];
  let plainRedoStack: { value: string; start: number; end: number }[] = [];
  let lastPlainUndoSnapshotAt = 0;
  const PLAIN_UNDO_COALESCE_MS = 500;

  // Resyncs from the parent's content and resets undo history only when the
  // *selected note* changes - not on every keystroke, matching how
  // MarkdownEditor only calls setContent on a noteId change rather than
  // syncing continuously off the content prop.
  $: if (noteId !== lastNoteId) {
    value = content;
    plainUndoStack = [];
    plainRedoStack = [];
    lastNoteId = noteId;
  }

  // Line numbers are per logical line (split on \n), not per wrapped visual
  // row - matching how editors typically define "line numbers" (e.g. the
  // line a cursor position refers to), and avoiding the cost/fragility of
  // measuring where a plain <textarea> actually wraps text, which would
  // need to be recomputed on every resize as well as every edit. The plain
  // editor switches to no-wrap/horizontal-scroll while this is on (see the
  // .no-wrap class below) specifically so each logical line always renders
  // as exactly one row, keeping numbers correctly aligned with their line -
  // without that, a wrapped line would silently throw off every number
  // beneath it.
  $: plainLineNumberText = showLineNumbers ? Array.from({ length: value.split('\n').length }, (_, i) => i + 1).join('\n') : '';

  // Keeps the gutter's vertical position matched to the textarea's own
  // scroll - re-runs whenever the gutter is (re)mounted too, so toggling
  // the setting on while already scrolled down doesn't leave the gutter
  // stuck at the top until the next scroll event.
  $: if (gutterEl && textarea) {
    gutterEl.scrollTop = textarea.scrollTop;
  }

  const syncGutterScroll = () => {
    if (gutterEl && textarea) gutterEl.scrollTop = textarea.scrollTop;
  };

  const emitUpdate = () => onUpdate(value);

  export function focus() {
    textarea?.focus();
  }

  export function insertAtCursor(text: string) {
    const start = textarea.selectionStart;
    const end = textarea.selectionEnd;
    pushPlainUndoSnapshot(true);
    value = `${value.slice(0, start)}${text}${value.slice(end)}`;
    requestAnimationFrame(() => {
      const cursor = start + text.length;
      textarea.focus();
      textarea.setSelectionRange(cursor, cursor);
    });
    emitUpdate();
  }

  // Records a checkpoint to undo back to. `force` is for discrete
  // programmatic edits (paste, insert-timestamp/-divider/-dateline) that
  // should always be their own undo step; native typing goes through the
  // keydown handler below without force, so a burst of consecutive
  // keystrokes within PLAIN_UNDO_COALESCE_MS collapses into a single step
  // (matching how native undo normally groups continuous typing) instead of
  // undoing one character at a time. Snapshotting is driven by keydown
  // (fires before the keystroke's edit is applied) rather than the newer
  // beforeinput event, since beforeinput support on plain <textarea>
  // elements (as opposed to contenteditable) has historically been
  // inconsistent on WebKitGTK.
  const pushPlainUndoSnapshot = (force = false) => {
    const now = Date.now();
    if (!force && plainUndoStack.length && now - lastPlainUndoSnapshotAt < PLAIN_UNDO_COALESCE_MS) {
      lastPlainUndoSnapshotAt = now;
      return;
    }
    plainUndoStack = [
      ...plainUndoStack.slice(-199),
      { value, start: textarea?.selectionStart ?? value.length, end: textarea?.selectionEnd ?? value.length },
    ];
    plainRedoStack = [];
    lastPlainUndoSnapshotAt = now;
  };

  const undoPlainText = () => {
    if (!plainUndoStack.length) return;
    const current = { value, start: textarea.selectionStart, end: textarea.selectionEnd };
    const prev = plainUndoStack[plainUndoStack.length - 1];
    plainUndoStack = plainUndoStack.slice(0, -1);
    plainRedoStack = [...plainRedoStack, current];
    value = prev.value;
    requestAnimationFrame(() => {
      textarea.focus();
      textarea.setSelectionRange(prev.start, prev.end);
    });
    emitUpdate();
  };

  const redoPlainText = () => {
    if (!plainRedoStack.length) return;
    const current = { value, start: textarea.selectionStart, end: textarea.selectionEnd };
    const next = plainRedoStack[plainRedoStack.length - 1];
    plainRedoStack = plainRedoStack.slice(0, -1);
    plainUndoStack = [...plainUndoStack, current];
    value = next.value;
    requestAnimationFrame(() => {
      textarea.focus();
      textarea.setSelectionRange(next.start, next.end);
    });
    emitUpdate();
  };

  // Keys that don't modify the field's content - no undo checkpoint needed
  // for these (also avoids fighting the tree/search's own arrow-key
  // navigation shortcuts).
  const NON_EDITING_KEYS = new Set([
    'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Home', 'End', 'PageUp', 'PageDown',
    'Shift', 'Control', 'Alt', 'Meta', 'CapsLock', 'Tab', 'Escape',
    'F1', 'F2', 'F3', 'F4', 'F5', 'F6', 'F7', 'F8', 'F9', 'F10', 'F11', 'F12',
  ]);

  const handleKeydown = (event: KeyboardEvent) => {
    if (!editable) return;
    if (event.ctrlKey || event.metaKey) {
      const key = event.key.toLowerCase();
      if (key === 'z' && !event.shiftKey) {
        event.preventDefault();
        undoPlainText();
      } else if ((key === 'z' && event.shiftKey) || key === 'y') {
        event.preventDefault();
        redoPlainText();
      }
      return;
    }
    if (!NON_EDITING_KEYS.has(event.key)) pushPlainUndoSnapshot(false);
  };

  // Plain-text notes should never pick up rich formatting from the
  // clipboard (bold/colors/fonts from a pasted webpage, Word doc, etc.) -
  // force the text/plain flavor regardless of how Ctrl+V or the OS's own
  // "Paste" action populated the clipboard. Markdown notes keep the
  // editor's normal paste handling, which is expected to preserve
  // formatting.
  //
  // Even the text/plain flavor isn't clean, though: when copying from a
  // rendered webpage, browsers generate that plain-text fallback from the
  // page's DOM structure, which bakes in each source element's indentation
  // as literal leading spaces/tabs on every line - strip those per line so
  // pasted lines start flush left, matching what plain notes expect.
  const handlePaste = (event: ClipboardEvent) => {
    event.preventDefault();
    const raw = event.clipboardData?.getData('text/plain') ?? '';
    const text = raw
      .split('\n')
      .map((line) => line.replace(/^[ \t]+/, ''))
      .join('\n');
    insertAtCursor(text);
  };
</script>

<div class="plain-editor-wrap">
  {#if showLineNumbers}
    <pre class="line-gutter" bind:this={gutterEl} aria-hidden="true">{plainLineNumberText}</pre>
  {/if}
  <textarea
    bind:this={textarea}
    bind:value
    class="editor"
    class:no-wrap={showLineNumbers}
    {placeholder}
    readonly={!editable}
    on:input={emitUpdate}
    on:paste={handlePaste}
    on:keydown={handleKeydown}
    on:scroll={syncGutterScroll}
  ></textarea>
</div>

<style>
  .plain-editor-wrap {
    flex: 1;
    display: flex;
    min-height: 0;
  }

  .editor {
    flex: 1;
    width: 100%;
    min-width: 0;
    border: 0;
    resize: none;
    outline: none;
    padding: 1rem 1.1rem;
    background: transparent;
    color: inherit;
    line-height: 1.55;
  }

  .editor::placeholder {
    color: var(--muted);
    opacity: 1;
  }

  /* Wrapping is disabled while the gutter is showing (see .no-wrap below) so
     every logical line renders as exactly one row - required for the
     line-number-per-logical-line approach above to actually line up with
     its text; a wrapped line would otherwise push every number beneath it
     out of alignment. */
  .editor.no-wrap {
    white-space: pre;
    overflow-x: auto;
  }

  .line-gutter {
    flex-shrink: 0;
    margin: 0;
    min-width: 2ch;
    padding: 1rem 0.6rem 1rem 0.7rem;
    font-family: inherit;
    /* Smaller than the note text (a smaller, quieter rail reads better than
       numbers competing at the same size), but line-height is set in rem
       (absolute, independent of this element's own smaller font-size)
       rather than as a unitless multiplier, so each row still lines up
       exactly with .editor's own 1.55 line-height despite the smaller
       font. No background tint anymore either - that plus the border was
       what made the gutter read as a heavy, separate block instead of a
       thin rail. */
    font-size: 0.75em;
    font-variant-numeric: tabular-nums;
    line-height: 1.55rem;
    color: var(--muted);
    text-align: right;
    user-select: none;
    overflow: hidden;
    white-space: pre;
    border-right: 1px solid var(--border);
    opacity: 0.8;
  }
</style>

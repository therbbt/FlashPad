<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { Editor, Extension } from '@tiptap/core';
  import { Plugin, TextSelection } from '@tiptap/pm/state';
  import type { EditorView } from '@tiptap/pm/view';
  import StarterKit from '@tiptap/starter-kit';
  import TiptapLink from '@tiptap/extension-link';
  import TiptapImage from '@tiptap/extension-image';
  import Placeholder from '@tiptap/extension-placeholder';
  import TaskList from '@tiptap/extension-task-list';
  import TaskItem from '@tiptap/extension-task-item';
  import { Markdown } from 'tiptap-markdown';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import { isAllowedLinkUrl } from '../utils/links';
  import { isAllowedImageMimeType, isAllowedImagePath } from '../utils/images';
  import { readDroppedImage } from '../services/imagesService';
  import { vimModeIndicator } from '../stores/vimModeIndicator';

  const isTauriRuntime = () => typeof window !== 'undefined' && Boolean((window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);

  export let content: string;
  export let noteId: number;
  export let onUpdate: (markdown: string) => void;
  // Called with a link's href on click (locked notes: plain click: unlocked
  // notes: Ctrl/Cmd+click - see the handleClick editorProps below) or Alt+O.
  // App.svelte owns the actual opening + protocol re-validation + error
  // toast, since it's the one place that needs to do that consistently for
  // both the click handler here and the right-click "Open link" menu item.
  export let onOpenLink: (url: string) => void;
  export let placeholder = '';
  export let editable = true;
  export let vimMode = false;

  let element: HTMLDivElement;
  let editor: Editor | undefined;
  let lastNoteId: number | undefined;

  // Extended purely to make the tooltip show the full URL (title=href) -
  // native anchor tooltips need a real title attribute, and Tiptap doesn't
  // set one by default. Delegates to the base extension's renderHTML
  // (this.parent) for everything else, including the isAllowedUri-based
  // href-blanking for disallowed protocols.
  const Link = TiptapLink.extend({
    renderHTML(props) {
      const [tag, attrs, content] = (this.parent?.(props) ?? ['a', props.HTMLAttributes, 0]) as [string, Record<string, unknown>, number];
      return [tag, { ...attrs, title: attrs.title ?? attrs.href ?? null }, content];
    },
  });

  const escapeHtmlAttr = (value: string) => value.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

  // Extended so a resized image's width/height survive being saved and
  // reloaded. tiptap-markdown does NOT use @tiptap/extension-image's own
  // parseMarkdown/renderMarkdown fields - it looks up serialization by node
  // NAME in its own bundled registry (tiptap-markdown/extensions/nodes/
  // image.js), which just delegates to prosemirror-markdown's default image
  // serializer (plain `![alt](src)`, no size). addStorage() here overrides
  // that lookup for this schema's "image" node specifically. Plain markdown
  // has no syntax for width/height, so once an image has been resized we
  // fall back to serializing it as a raw <img> tag instead - tiptap-markdown
  // parses embedded HTML (html: true, the Markdown extension's default)
  // back into the doc via this node's own parseHTML(), which already reads
  // width/height like any other attribute, so the default (empty) parse
  // spec still works unchanged.
  const ResizableImage = TiptapImage.extend({
    addStorage() {
      return {
        markdown: {
          serialize(state: { write: (text: string) => void; esc: (text: string) => string; quote: (text: string) => string }, node: { attrs: Record<string, unknown> }) {
            const { src, alt, title, width, height } = node.attrs as {
              src?: string;
              alt?: string;
              title?: string;
              width?: number;
              height?: number;
            };
            if (!width && !height) {
              state.write(`![${state.esc(alt || '')}](${state.esc(src || '')}${title ? ` ${state.quote(title)}` : ''})`);
              return;
            }
            const attrs: [string, string | undefined][] = [
              ['src', src],
              ['alt', alt],
              ['title', title],
              ['width', width != null ? String(width) : undefined],
              ['height', height != null ? String(height) : undefined],
            ];
            const attrString = attrs
              .filter((entry): entry is [string, string] => Boolean(entry[1]))
              .map(([key, value]) => `${key}="${escapeHtmlAttr(value)}"`)
              .join(' ');
            state.write(`<img ${attrString}>`);
          },
          parse: {},
        },
      };
    },
  });

  // Minimal vim-lite modal editing for Markdown notes - deliberately NOT
  // the full vim feature set (no word motions, text objects, registers,
  // counts, dot-repeat, or search - see PlainTextEditor.svelte's
  // CodeMirror-based vim mode for that, which has a mature library behind
  // it). This is just enough for movement + starting a new line without
  // reaching for the mouse: normal/insert modes, hjkl, 0/$, i/o/O, x, dd.
  // Only i and o/O enter insert mode - no `a`/`A`/`I` etc. (a deliberate
  // trim from the fuller set a real vim would have, to keep the mode
  // switches predictable). Hand-rolled as a small ProseMirror plugin (via
  // a Tiptap Extension)
  // since nothing at this scale exists as a library for ProseMirror -
  // imports come from @tiptap/pm/* (Tiptap's own bundled ProseMirror
  // re-export) rather than a separate prosemirror-* package, so they're
  // guaranteed to be the exact classes Tiptap's editor instance uses.
  let vimNormalMode = true;
  let pendingDeleteBlock = false;
  let pendingDeleteBlockTimer: ReturnType<typeof setTimeout> | undefined;

  const enterInsertMode = () => {
    vimNormalMode = false;
  };

  const enterNormalMode = () => {
    vimNormalMode = true;
    pendingDeleteBlock = false;
  };

  const moveChar = (view: EditorView, dir: 1 | -1) => {
    const { doc, selection } = view.state;
    const pos = Math.max(0, Math.min(doc.content.size, selection.from + dir));
    view.dispatch(view.state.tr.setSelection(TextSelection.near(doc.resolve(pos), dir)));
  };

  // "Line" here means the current block (paragraph/heading/list item), same
  // as moveToBlockEdge below - j/k move to the next/previous block rather
  // than a wrapped screen line. (An earlier version used
  // coordsAtPos/posAtCoords to find a screen line above/below, but the
  // margin between blocks made the small pixel nudge unreliable - it often
  // resolved back inside the *same* block, so j/k effectively did nothing.
  // This structural approach, matching moveToBlockEdge, is exact instead of
  // approximate.)
  const moveBlock = (view: EditorView, dir: 1 | -1) => {
    const { doc } = view.state;
    const $pos = view.state.selection.$from;
    const depth = $pos.depth;
    const raw = dir > 0 ? $pos.after(depth) + 1 : $pos.before(depth) - 1;
    const pos = Math.max(0, Math.min(doc.content.size, raw));
    view.dispatch(view.state.tr.setSelection(TextSelection.near(doc.resolve(pos), dir)));
  };

  // "Line" here means the current block (paragraph/heading/list item) -
  // there's no plain-text-style single line to jump to in a rich doc.
  const moveToBlockEdge = (view: EditorView, edge: 'start' | 'end') => {
    const $pos = view.state.selection.$from;
    const pos = edge === 'start' ? $pos.start() : $pos.end();
    view.dispatch(view.state.tr.setSelection(TextSelection.create(view.state.doc, pos)));
  };

  // Splits the current block at its start/end so a new one opens above/
  // below, landing the cursor inside the new (empty) one - handles the
  // common cases (paragraphs, list items) via ProseMirror's own
  // schema-aware Transaction.split, but isn't a full replacement for
  // Tiptap's specialized Enter-key command chain (e.g. exiting an empty
  // list item); Enter itself is untouched and still available.
  const openBlock = (view: EditorView, where: 'below' | 'above') => {
    const $pos = view.state.selection.$from;
    const splitAt = where === 'below' ? $pos.end() : $pos.start();
    const bias = where === 'below' ? 1 : -1;
    const tr = view.state.tr;
    tr.split(splitAt);
    const mapped = tr.mapping.map(splitAt, bias);
    tr.setSelection(TextSelection.near(tr.doc.resolve(mapped), bias));
    view.dispatch(tr);
    enterInsertMode();
  };

  const deleteCharForward = (view: EditorView) => {
    const { from } = view.state.selection;
    if (from >= view.state.doc.content.size) return;
    view.dispatch(view.state.tr.delete(from, from + 1));
  };

  const deleteCurrentBlock = (view: EditorView) => {
    const $pos = view.state.selection.$from;
    const start = $pos.before($pos.depth);
    const end = $pos.after($pos.depth);
    const tr = view.state.tr;
    if (view.state.doc.childCount <= 1) {
      // Removing the only remaining block would leave an invalid empty
      // document - clear its content instead.
      tr.delete(start + 1, end - 1);
    } else {
      tr.delete(start, end);
    }
    view.dispatch(tr);
  };

  const VimLite = Extension.create({
    name: 'vimLite',
    addProseMirrorPlugins() {
      return [
        new Plugin({
          props: {
            handleKeyDown: (view, event) => {
              if (!vimMode || !editable) return false;
              if (!vimNormalMode) {
                if (event.key === 'Escape') {
                  enterNormalMode();
                  return true;
                }
                return false;
              }
              // Leave app/Tiptap modifier shortcuts (Alt+..., Cmd+B, etc.)
              // alone - vim-lite only claims plain, unmodified keys.
              if (event.ctrlKey || event.metaKey || event.altKey) return false;
              if (pendingDeleteBlock) {
                clearTimeout(pendingDeleteBlockTimer);
                pendingDeleteBlock = false;
                if (event.key === 'd') {
                  deleteCurrentBlock(view);
                  return true;
                }
                // Any other key just cancels the pending "d" (no generic
                // operator+motion composition) - fall through and handle
                // this key normally below.
              }
              switch (event.key) {
                case 'h':
                  moveChar(view, -1);
                  return true;
                case 'l':
                  moveChar(view, 1);
                  return true;
                case 'j':
                  moveBlock(view, 1);
                  return true;
                case 'k':
                  moveBlock(view, -1);
                  return true;
                case '0':
                  moveToBlockEdge(view, 'start');
                  return true;
                case '$':
                  moveToBlockEdge(view, 'end');
                  return true;
                case 'i':
                  enterInsertMode();
                  return true;
                case 'o':
                  openBlock(view, 'below');
                  return true;
                case 'O':
                  openBlock(view, 'above');
                  return true;
                case 'x':
                  deleteCharForward(view);
                  return true;
                case 'd':
                  pendingDeleteBlock = true;
                  pendingDeleteBlockTimer = setTimeout(() => {
                    pendingDeleteBlock = false;
                  }, 600);
                  return true;
                case 'Escape':
                  // Already in normal mode - no-op, but still fully
                  // swallowed (see stopEscapeWhileVimActive below for why).
                  return true;
                default:
                  // Swallow any other plain printable key so normal mode
                  // never leaks stray text into the document; navigation/
                  // editing keys we don't special-case (arrows, Backspace,
                  // Enter, Tab, Home/End, ...) are left alone.
                  return event.key.length === 1;
              }
            },
          },
        }),
      ];
    },
  });

  export function insertAtCursor(text: string) {
    if (!editable) return;
    editor?.chain().focus().insertContent(text).run();
  }

  export function focus() {
    editor?.chain().focus().run();
  }

  // Alt+O - returns the href of the link at the caret, or null if there
  // isn't one, so App.svelte can hand it to the same open+validate path
  // used by clicking and the context menu.
  export function getLinkHrefAtCursor(): string | null {
    if (!editor?.isActive('link')) return null;
    return (editor.getAttributes('link').href as string | undefined) ?? null;
  }

  // Handles link clicks via a real native "click" listener in the capture
  // phase, rather than Tiptap/ProseMirror's editorProps.handleClick.
  // ProseMirror reconstructs click semantics from its own mousedown/mouseup
  // tracking rather than listening for the browser's actual "click" event,
  // and the base Link extension's own bundled click plugin (still present -
  // we only override renderHTML above) explicitly defers to the native
  // anchor click whenever the view isn't editable. Between those two
  // things, preventDefault() called from inside editorProps.handleClick did
  // not reliably suppress the native anchor navigation in testing - a
  // direct, real "click" listener in the capture phase is the same
  // technique client-side routers use to intercept link clicks, and is
  // guaranteed to run against the actual event the browser's default
  // navigation is tied to.
  const handleLinkClick = (event: MouseEvent) => {
    const anchor = (event.target as HTMLElement | null)?.closest?.('a[href]') as HTMLAnchorElement | null;
    const href = anchor?.getAttribute('href');
    if (!href) return;
    // Always prevent the native anchor click-through first - without this,
    // a plain click on a link that we decide NOT to open (the "just place
    // the caret" case below) would still navigate, since the rendered
    // element is a real <a href>.
    event.preventDefault();
    event.stopPropagation();
    // Locked (non-editable) notes: any click opens the link, since there's
    // no caret to place and no text to edit. Unlocked notes: require
    // Ctrl/Cmd+click so a plain click on a link mid-sentence still just
    // moves the caret, keeping the link's text editable.
    if (editable && !(event.ctrlKey || event.metaKey)) return;
    onOpenLink(href);
  };

  // Applies a task item checkbox's new checked state to the document when
  // the note is locked (read-only). Unlocked notes don't need this - Tiptap
  // already updates the document correctly there via its own getPos()-based
  // command in the TaskItem node view.
  //
  // Locked notes are why this exists: Tiptap's onReadOnlyChecked hook only
  // receives the node, not its position, and the "obvious" fix - searching
  // the document for a node === match - turned out to be unreliable. The
  // TaskItem node view's change-listener closure captures `node` once at
  // node-view-creation time and never refreshes it on later edits, so for
  // any item whose text was typed after it was created (i.e. essentially
  // all of them), the node it hands back is a stale, content-less copy that
  // can never match the live document. Locating the position from the
  // actual clicked DOM element via view.posAtDOM sidesteps that entirely.
  const handleTaskCheckboxChange = (event: Event) => {
    if (editable) return;
    const checkbox = event.target as HTMLInputElement;
    if (checkbox.type !== 'checkbox') return;
    const listItem = checkbox.closest('li');
    if (!editor || !listItem) return;
    const domPos = editor.view.posAtDOM(listItem, 0);
    const $pos = editor.state.doc.resolve(domPos);
    for (let depth = $pos.depth; depth > 0; depth--) {
      const candidate = $pos.node(depth);
      if (candidate.type.name === 'taskItem') {
        const tr = editor.state.tr.setNodeMarkup($pos.before(depth), undefined, { ...candidate.attrs, checked: checkbox.checked });
        // Deliberately left out of undo history: Ctrl+Z can't reach this
        // note while it's locked anyway (ProseMirror skips keydown
        // handling entirely for non-editable views), and letting a toggle
        // made while locked sit in the stack to surface later - after
        // unlocking and editing normally - would be a confusing
        // action-at-a-distance. Ticking the box again is the toggle's own
        // undo.
        tr.setMeta('addToHistory', false);
        editor.view.dispatch(tr);
        return;
      }
    }
  };

  // Images are embedded directly in the note as base64 data: URIs rather
  // than saved as separate files - the whole app is built around "one
  // .sqlite3 file is a complete, portable database" (multi-database
  // support, backup, export/import all assume this), and a data URI keeps
  // that invariant fully intact with no schema/storage changes at all.
  const MAX_IMAGE_DIMENSION = 1600;
  const DOWNSCALED_JPEG_QUALITY = 0.85;

  const readAsDataUrl = (blob: Blob): Promise<string> =>
    new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(reader.result as string);
      reader.onerror = () => reject(reader.error);
      reader.readAsDataURL(blob);
    });

  // Downscales only if the image actually exceeds MAX_IMAGE_DIMENSION on
  // either side - otherwise reads the original bytes untouched, preserving
  // quality and (for GIFs) animation exactly. An oversized GIF is the one
  // exception: canvas can only ever capture a single frame, so downscaling
  // one necessarily flattens its animation - re-encoded as PNG in that case
  // to at least keep transparency, rather than as a static "GIF".
  const toEmbeddableDataUrl = async (file: File): Promise<string> => {
    let bitmap: ImageBitmap;
    try {
      bitmap = await createImageBitmap(file);
    } catch {
      // Decode failed for some reason - embed the original bytes rather
      // than dropping the paste/drop entirely.
      return readAsDataUrl(file);
    }
    try {
      if (bitmap.width <= MAX_IMAGE_DIMENSION && bitmap.height <= MAX_IMAGE_DIMENSION) {
        return await readAsDataUrl(file);
      }
      const scale = MAX_IMAGE_DIMENSION / Math.max(bitmap.width, bitmap.height);
      const canvas = document.createElement('canvas');
      canvas.width = Math.round(bitmap.width * scale);
      canvas.height = Math.round(bitmap.height * scale);
      const ctx = canvas.getContext('2d');
      if (!ctx) return await readAsDataUrl(file);
      ctx.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
      const outputType = file.type === 'image/gif' ? 'image/png' : file.type;
      return canvas.toDataURL(outputType, DOWNSCALED_JPEG_QUALITY);
    } finally {
      bitmap.close();
    }
  };

  const insertImageFile = async (file: File) => {
    if (!isAllowedImageMimeType(file.type)) return;
    const src = await toEmbeddableDataUrl(file);
    editor?.chain().focus().setImage({ src, alt: file.name || '' }).run();
  };

  // Real desktop drag-and-drop (from a file manager, over Tauri's native
  // onDragDropEvent - see below) only ever gives us a filesystem path, never
  // a File with real bytes: on Linux/WebKitGTK the DOM's own `drop` event
  // carries no file payload at all for OS-originated drags (only a
  // text/uri-list string), so we read the bytes on the Rust side instead and
  // route the resulting data: URI through the same downscale pipeline as a
  // pasted file.
  const insertImageFromPath = async (path: string) => {
    if (!editable) return;
    try {
      const dataUrl = await readDroppedImage(path);
      const blob = await (await fetch(dataUrl)).blob();
      const name = path.split(/[\\/]/).pop() || 'image';
      await insertImageFile(new File([blob], name, { type: blob.type }));
    } catch (error) {
      console.error('Failed to embed dropped image', error);
    }
  };

  // Only intercepts when the clipboard/drop actually contains an image -
  // anything else (plain text, etc.) is left completely alone so normal
  // paste/drop behavior is unaffected.
  const handleImagePaste = (event: ClipboardEvent) => {
    if (!editable) return;
    const items = event.clipboardData?.items;
    if (!items) return;
    for (const item of items) {
      if (item.kind === 'file' && isAllowedImageMimeType(item.type)) {
        const file = item.getAsFile();
        if (!file) continue;
        event.preventDefault();
        void insertImageFile(file);
        return;
      }
    }
  };

  const handleImageDrop = (event: DragEvent) => {
    if (!editable) return;
    const files = event.dataTransfer?.files;
    if (!files) return;
    const imageFiles = Array.from(files).filter((file) => isAllowedImageMimeType(file.type));
    if (!imageFiles.length) return;
    event.preventDefault();
    for (const file of imageFiles) {
      void insertImageFile(file);
    }
  };

  // Only set when running under Tauri (see isTauriRuntime above) - unused in
  // the plain-browser dev/preview fallback, which relies on handleImageDrop
  // (the DOM 'drop' listener below) instead.
  let unlistenDragDrop: (() => void) | undefined;

  // Vim-lite's own Escape (leave insert mode) must not bubble to
  // App.svelte's global window keydown handler, which hides the whole app
  // window on a bare Escape when no dialog is open - same fix as
  // PlainTextEditor.svelte's vim mode. Attached to `element` (an ancestor
  // of the actual contenteditable ProseMirror renders into), so it runs
  // after ProseMirror's own handling but before the event can bubble any
  // further. Only active while vim mode is on and the note is editable -
  // Escape keeps bubbling (today's behavior) otherwise.
  const stopEscapeWhileVimActive = (event: KeyboardEvent) => {
    if (vimMode && editable && event.key === 'Escape') event.stopPropagation();
  };

  onMount(() => {
    element.addEventListener('click', handleLinkClick, true);
    element.addEventListener('change', handleTaskCheckboxChange);
    element.addEventListener('paste', handleImagePaste, true);
    element.addEventListener('drop', handleImageDrop, true);
    element.addEventListener('keydown', stopEscapeWhileVimActive);

    if (isTauriRuntime()) {
      void getCurrentWebviewWindow()
        .onDragDropEvent((event) => {
          if (event.payload.type !== 'drop') return;
          for (const path of event.payload.paths) {
            if (isAllowedImagePath(path)) void insertImageFromPath(path);
          }
        })
        .then((unlisten) => {
          unlistenDragDrop = unlisten;
        });
    }

    editor = new Editor({
      element,
      editable,
      extensions: [
        // Listed first so its handleKeyDown gets first crack at every key -
        // same precedence requirement as the plain-text editor's "vim must
        // come before other keymaps".
        VimLite,
        // link: false - StarterKit bundles its own Link instance under the
        // same "link" mark name, which would otherwise collide with the
        // configured one below.
        StarterKit.configure({ link: false }),
        Link.configure({
          // We own opening links ourselves (see handleLinkClick above) so
          // plain clicks in an editable note still just place the caret -
          // Tiptap's built-in openOnClick can't distinguish that from a
          // deliberate Ctrl/Cmd+click.
          openOnClick: false,
          autolink: true,
          protocols: ['http', 'https', 'mailto'],
          isAllowedUri: isAllowedLinkUrl,
          validate: isAllowedLinkUrl,
          // target explicitly nulled out (Tiptap's default is "_blank") -
          // it's what enables the browser's native middle-click/auxclick
          // "open in new tab" gesture, which bypasses handleClick entirely
          // since that's a different event than a regular click. We own
          // opening links ourselves; there's no click path we want the
          // native anchor behavior to handle.
          HTMLAttributes: { rel: 'noopener noreferrer nofollow', target: null },
        }),
        ResizableImage.configure({
          inline: false,
          // We construct our own data: URIs (see toEmbeddableDataUrl above)
          // rather than letting arbitrary pasted HTML through, but this
          // still needs enabling - the extension's default parseHTML
          // rejects `data:` src values otherwise.
          allowBase64: true,
          HTMLAttributes: { loading: 'lazy' },
          // Built into @tiptap/extension-image - draws corner drag handles
          // (styled below) and persists the final size via updateAttributes,
          // which renderMarkdown above then serializes.
          resize: { enabled: true, minWidth: 40, minHeight: 40 },
        }),
        Placeholder.configure({ placeholder }),
        TaskList,
        TaskItem.configure({
          nested: true,
          // Locking a note protects its TEXT, not a checklist's state - the
          // notes people actually lock are often finished checklists they
          // still want to tick items off of. Tiptap already renders the
          // checkbox as a real, non-disabled <input> even when read-only
          // (only the surrounding text becomes non-editable, since it's a
          // separate DOM subtree the checkbox's own click can't reach) - by
          // default it just reverts the DOM checkbox back on every click
          // unless this option is set. The actual document update happens
          // in handleTaskCheckboxChange above (see its comment for why);
          // this just needs to return true so Tiptap doesn't revert the DOM
          // checkbox out from under that.
          onReadOnlyChecked: () => true,
        }),
        // breaks: true keeps a single newline as a hard line break (matching
        // the plain textarea) instead of CommonMark's default of collapsing
        // it into a soft space, so existing plain content doesn't visually
        // reflow the first time a note is switched into markdown mode.
        // linkify: true turns bare URLs in the markdown source into real
        // links (GFM autolink), matching GitHub's rendering.
        Markdown.configure({ breaks: true, linkify: true }),
      ],
      content,
      onUpdate: ({ editor: instance }) => {
        onUpdate(instance.storage.markdown.getMarkdown());
      },
    });
    lastNoteId = noteId;
  });

  // Only resync when the *selected note* changes - not on every keystroke,
  // which would otherwise fight the user's own typing/cursor position since
  // `content` is also updated (via onUpdate above) as a side effect of typing.
  $: if (editor && noteId !== lastNoteId) {
    editor.commands.setContent(content, false);
    lastNoteId = noteId;
    // Vim buffers always start in normal mode.
    enterNormalMode();
  }

  $: editor?.setEditable(editable);

  // Single source of truth for the footer badge - reacts to the mode
  // switching (vimNormalMode), the setting toggling, and the note's
  // editable/locked state, so none of those call sites need to touch the
  // store directly. Only one of MarkdownEditor/PlainTextEditor is ever
  // mounted at a time, so there's no cross-editor contention over it.
  $: vimModeIndicator.set(editor && vimMode && editable ? (vimNormalMode ? 'NORMAL' : 'INSERT') : null);

  onDestroy(() => {
    element.removeEventListener('click', handleLinkClick, true);
    element.removeEventListener('change', handleTaskCheckboxChange);
    element.removeEventListener('paste', handleImagePaste, true);
    element.removeEventListener('drop', handleImageDrop, true);
    element.removeEventListener('keydown', stopEscapeWhileVimActive);
    unlistenDragDrop?.();
    vimModeIndicator.set(null);
    clearTimeout(pendingDeleteBlockTimer);
    editor?.destroy();
  });
</script>

<div class="markdown-editor" bind:this={element}></div>

<style>
  .markdown-editor {
    flex: 1;
    overflow: auto;
    min-height: 0;
  }

  .markdown-editor :global(.tiptap) {
    min-height: 100%;
    padding: 1rem 1.1rem 4rem;
    outline: none;
    line-height: 1.55;
    color: inherit;
  }

  .markdown-editor :global(.tiptap > *:first-child) {
    margin-top: 0;
  }

  .markdown-editor :global(.tiptap > *:last-child) {
    margin-bottom: 0;
  }

  .markdown-editor :global(.tiptap p.is-editor-empty:first-child::before) {
    content: attr(data-placeholder);
    float: left;
    height: 0;
    pointer-events: none;
    color: var(--muted);
  }

  .markdown-editor :global(.tiptap h1),
  .markdown-editor :global(.tiptap h2),
  .markdown-editor :global(.tiptap h3) {
    margin: 0.6em 0 0.3em;
    line-height: 1.3;
  }

  .markdown-editor :global(.tiptap h1) {
    font-size: 1.5em;
  }

  .markdown-editor :global(.tiptap h2) {
    font-size: 1.25em;
  }

  .markdown-editor :global(.tiptap h3) {
    font-size: 1.1em;
  }

  .markdown-editor :global(.tiptap p) {
    margin: 0.3em 0;
  }

  .markdown-editor :global(.tiptap strong) {
    color: var(--text);
  }

  .markdown-editor :global(.tiptap code) {
    background: var(--panel-2);
    border-radius: 0.25rem;
    padding: 0.1em 0.3em;
    font-size: 0.9em;
  }

  .markdown-editor :global(.tiptap pre) {
    background: var(--panel-2);
    border-radius: 0.4rem;
    padding: 0.6em 0.8em;
    overflow-x: auto;
  }

  .markdown-editor :global(.tiptap pre code) {
    background: none;
    padding: 0;
  }

  .markdown-editor :global(.tiptap blockquote) {
    margin: 0.4em 0;
    padding-left: 0.8em;
    border-left: 3px solid var(--border);
    color: var(--muted);
  }

  .markdown-editor :global(.tiptap ul),
  .markdown-editor :global(.tiptap ol) {
    padding-left: 1.4em;
    margin: 0.3em 0;
  }

  .markdown-editor :global(.tiptap a) {
    color: var(--accent);
    text-decoration: none;
    cursor: pointer;
  }

  .markdown-editor :global(.tiptap a:hover) {
    text-decoration: underline;
  }

  .markdown-editor :global(.tiptap ul[data-type='taskList']) {
    padding-left: 0.2em;
    list-style: none;
  }

  .markdown-editor :global(.tiptap ul[data-type='taskList'] li) {
    display: flex;
    align-items: flex-start;
    gap: 0.5em;
  }

  .markdown-editor :global(.tiptap ul[data-type='taskList'] li > label) {
    display: flex;
    margin-top: 0.35em;
    user-select: none;
  }

  .markdown-editor :global(.tiptap ul[data-type='taskList'] li > label input[type='checkbox']) {
    margin: 0;
    accent-color: var(--accent);
    cursor: pointer;
  }

  .markdown-editor :global(.tiptap ul[data-type='taskList'] li > div) {
    flex: 1;
  }

  .markdown-editor :global(.tiptap ul[data-type='taskList'] li > div > p) {
    margin: 0;
  }

  .markdown-editor :global(.tiptap hr) {
    border: none;
    border-top: 1px solid var(--border);
    margin: 0.8em 0;
  }

  .markdown-editor :global(.tiptap img) {
    max-width: 100%;
    height: auto;
    border-radius: 0.3rem;
    display: block;
    margin: 0.4em 0;
  }

  /* Resize handles come from @tiptap/extension-image's built-in
     ResizableNodeView (see ResizableImage above) - it wraps each image in
     [data-resize-container] > [data-resize-wrapper] > img and creates
     unstyled [data-resize-handle] divs, positioned via inline styles
     (top/right/bottom/left: 0) but otherwise bare, so all visual styling
     lives here. */
  .markdown-editor :global([data-resize-container]) {
    max-width: 100%;
  }

  .markdown-editor :global([data-resize-handle]) {
    width: 0.6rem;
    height: 0.6rem;
    background: var(--accent);
    border: 1.5px solid var(--panel);
    border-radius: 2px;
    opacity: 0;
    transition: opacity 0.1s ease;
  }

  .markdown-editor :global([data-resize-wrapper]:hover [data-resize-handle]) {
    opacity: 1;
  }

  .markdown-editor :global([data-resize-handle='top-left']) {
    cursor: nwse-resize;
    transform: translate(-50%, -50%);
  }

  .markdown-editor :global([data-resize-handle='bottom-right']) {
    cursor: nwse-resize;
    transform: translate(50%, 50%);
  }

  .markdown-editor :global([data-resize-handle='top-right']) {
    cursor: nesw-resize;
    transform: translate(50%, -50%);
  }

  .markdown-editor :global([data-resize-handle='bottom-left']) {
    cursor: nesw-resize;
    transform: translate(-50%, 50%);
  }
</style>

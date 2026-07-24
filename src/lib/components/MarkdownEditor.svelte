<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { Editor } from '@tiptap/core';
  import StarterKit from '@tiptap/starter-kit';
  import TiptapLink from '@tiptap/extension-link';
  import Placeholder from '@tiptap/extension-placeholder';
  import TaskList from '@tiptap/extension-task-list';
  import TaskItem from '@tiptap/extension-task-item';
  import { Markdown } from 'tiptap-markdown';
  import { isAllowedLinkUrl } from '../utils/links';

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

  onMount(() => {
    editor = new Editor({
      element,
      editable,
      extensions: [
        // link: false - StarterKit bundles its own Link instance under the
        // same "link" mark name, which would otherwise collide with the
        // configured one below.
        StarterKit.configure({ link: false }),
        Link.configure({
          // We own opening links ourselves (see editorProps.handleClick
          // below) so plain clicks in an editable note still just place the
          // caret - Tiptap's built-in openOnClick can't distinguish that
          // from a deliberate Ctrl/Cmd+click.
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
        Placeholder.configure({ placeholder }),
        TaskList,
        TaskItem.configure({ nested: true }),
        // breaks: true keeps a single newline as a hard line break (matching
        // the plain textarea) instead of CommonMark's default of collapsing
        // it into a soft space, so existing plain content doesn't visually
        // reflow the first time a note is switched into markdown mode.
        // linkify: true turns bare URLs in the markdown source into real
        // links (GFM autolink), matching GitHub's rendering.
        Markdown.configure({ breaks: true, linkify: true }),
      ],
      editorProps: {
        handleClick(_view, _pos, event) {
          const anchor = (event.target as HTMLElement | null)?.closest?.('a[href]') as HTMLAnchorElement | null;
          const href = anchor?.getAttribute('href');
          if (!href) return false;
          // Always prevent the native anchor click-through first - without
          // this, a plain click on a link that we decide NOT to open (the
          // "just place the caret" case below) would still fall through to
          // the browser's own default action and actually navigate/open a
          // new window, since the rendered element is a real <a href>.
          // Returning false (not true) afterwards still lets ProseMirror
          // handle normal caret placement for that case.
          event.preventDefault();
          // Locked (non-editable) notes: any click opens the link, since
          // there's no caret to place and no text to edit. Unlocked notes:
          // require Ctrl/Cmd+click so a plain click on a link mid-sentence
          // still just moves the caret, keeping the link's text editable.
          if (editable && !(event.ctrlKey || event.metaKey)) return false;
          onOpenLink(href);
          return true;
        },
      },
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
  }

  $: editor?.setEditable(editable);

  onDestroy(() => {
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
</style>

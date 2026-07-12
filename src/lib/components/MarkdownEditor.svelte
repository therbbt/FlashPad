<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { Editor } from '@tiptap/core';
  import StarterKit from '@tiptap/starter-kit';
  import Placeholder from '@tiptap/extension-placeholder';
  import { Markdown } from 'tiptap-markdown';

  export let content: string;
  export let noteId: number;
  export let onUpdate: (markdown: string) => void;
  export let placeholder = '';
  export let editable = true;

  let element: HTMLDivElement;
  let editor: Editor | undefined;
  let lastNoteId: number | undefined;

  export function insertAtCursor(text: string) {
    if (!editable) return;
    editor?.chain().focus().insertContent(text).run();
  }

  onMount(() => {
    editor = new Editor({
      element,
      editable,
      extensions: [
        StarterKit,
        Placeholder.configure({ placeholder }),
        // breaks: true keeps a single newline as a hard line break (matching
        // the plain textarea) instead of CommonMark's default of collapsing
        // it into a soft space, so existing plain content doesn't visually
        // reflow the first time a note is switched into markdown mode.
        Markdown.configure({ breaks: true }),
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

  .markdown-editor :global(.tiptap hr) {
    border: none;
    border-top: 1px solid var(--border);
    margin: 0.8em 0;
  }
</style>

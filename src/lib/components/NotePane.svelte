<script lang="ts">
  import { tick } from 'svelte';
  import type { NoteRecord } from '../services/notesService';
  import * as notesStore from '../stores/notesStore';
  import { notes } from '../stores/notesStore';
  import { status } from '../stores/statusStore';
  import { resolveEffectiveLanguage } from '../utils/languageDetect';
  import { computeBacklinks } from '../utils/wikiLinks';
  import { writeText as writeClipboardText, readText } from '@tauri-apps/plugin-clipboard-manager';
  import { getEditorContextMenuItemsFor, type EditorContext } from '../plugins/pluginApi';
  import NoteInfoPopover from './NoteInfoPopover.svelte';
  import ContextMenu, { type ContextMenuItem } from './ContextMenu.svelte';
  import MarkdownEditor from './MarkdownEditor.svelte';
  import PlainTextEditor from './PlainTextEditor.svelte';
  import EditorModeEditor from './EditorModeEditor.svelte';

  // One note's full editing surface (title, content, mode toggles, right-
  // click menu, autosave) - extracted out of App.svelte so both the primary
  // and (split view's) secondary pane can share exactly one implementation
  // instead of App.svelte owning two divergent copies of this logic. See
  // the "split view" plan for the extraction rationale.

  export let noteId: number | null;
  // Called with a note id this pane should switch to showing - wiki-link
  // clicks and backlink-popover clicks both resolve internally to a note id
  // and hand it off here, rather than each pane owning its own note-lookup/
  // create-if-missing logic. App.svelte decides what "switch to this note"
  // means for THIS pane instance (the primary pane also updates the global
  // selectedId/activeParentId stores; a split secondary pane would just
  // reassign its own noteId).
  export let onNavigate: (id: number) => void;
  // External (http/https/mailto) link clicks - App.svelte owns re-
  // validating the URL and handing it to the OS opener, since that's
  // shared, non-pane-specific behavior.
  export let onOpenLink: (url: string) => void;
  export let vimMode = false;
  export let theme: 'light' | 'dark' = 'dark';
  // Only meaningful in split view (a single pane is always implicitly
  // "focused") - whether this is the pane keyboard shortcuts/sidebar
  // clicks currently target. onFocus fires on mousedown, in the capture
  // phase, so clicking anywhere in this pane - including straight into its
  // editor - marks it focused before the click's own handling proceeds.
  export let focused = true;
  export let onFocus: () => void = () => {};

  // Bindable: App.svelte's ActionToolbar/Footer need to reflect these live
  // (which buttons show, whether the Markdown-guide/Format buttons are
  // relevant) without waiting on a save round-trip - see the "isLockedActive
  // isn't optimistic but isMarkdownActive/isEditorModeActive are" note below
  // for why these three are handled differently from each other.
  export let isMarkdownActive = false;
  export let isEditorModeActive = false;
  export let isLockedActive = false;

  let title = 'Untitled';
  let noteText = '';
  let titleAutoDerive = true;
  let showLineNumbersActive = false;

  let markdownEditorRef: MarkdownEditor | undefined;
  let plainEditorRef: PlainTextEditor | undefined;
  let editorModeEditorRef: EditorModeEditor | undefined;
  // The single dispatch point for the three text-editing methods every
  // editor implements the same way (focus/insertAtCursor/format) -
  // isEditorModeActive overrides isMarkdownActive entirely when on,
  // matching how the render block below picks which editor is mounted.
  const activeTextEditorRef = () => (isEditorModeActive ? editorModeEditorRef : isMarkdownActive ? markdownEditorRef : plainEditorRef);

  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  let contextMenu: { x: number; y: number; items: ContextMenuItem[] } | null = null;
  const closeContextMenu = () => {
    contextMenu = null;
  };

  let lastNoteId: number | null | undefined;
  $: currentNoteRecord = noteId == null ? null : ($notes.find((n) => n.id === noteId) ?? null);

  // isLockedActive and showLineNumbersActive are plain reactive derivations
  // (not optimistic local state, unlike isMarkdownActive/isEditorModeActive
  // below) - notesStore.toggleLock/toggleLineNumbers already update the
  // `notes` store as part of the same round trip App.svelte's old resync
  // code waited on, so deriving directly is behaviorally identical to that
  // resync, just automatic - and it means a note open in both panes at once
  // (split view) stays in sync between them for free, with no cross-pane
  // wiring needed.
  $: isLockedActive = currentNoteRecord?.isLocked ?? false;
  $: showLineNumbersActive = currentNoteRecord?.showLineNumbers ?? false;
  $: selectedNoteLanguage = currentNoteRecord?.language ?? null;
  $: selectedNoteEffectiveLanguage = noteId == null ? null : resolveEffectiveLanguage(noteText, isMarkdownActive, selectedNoteLanguage);
  $: selectedNoteBacklinks = currentNoteRecord ? computeBacklinks(currentNoteRecord, $notes) : [];

  // Loads title/content/mode flags fresh whenever this pane switches to a
  // different note (noteId prop change) - mirrors exactly how
  // PlainTextEditor/MarkdownEditor/EditorModeEditor already resync on their
  // own noteId prop one level down. isMarkdownActive/isEditorModeActive are
  // loaded here but NOT kept in sync reactively afterwards (unlike
  // isLockedActive/showLineNumbersActive above) - toggling either swaps
  // which editor component is mounted and needs to update instantly plus
  // chase focus into the new one (see toggleMarkdown/toggleEditorMode
  // below), which a save-round-trip-driven derivation can't do without a
  // perceptible lag.
  $: if (noteId !== lastNoteId) {
    if (currentNoteRecord) {
      title = currentNoteRecord.title;
      noteText = currentNoteRecord.content;
      isMarkdownActive = currentNoteRecord.isMarkdown;
      isEditorModeActive = currentNoteRecord.isEditorMode;
      // Date/time-named notes (see notesStore.createNoteIn) are NOT auto-
      // derived from typed content - only a truly untitled note is, so the
      // timestamp name sticks around as a stable identifier unless renamed
      // manually.
      titleAutoDerive = currentNoteRecord.title === 'Untitled' || currentNoteRecord.title.trim() === '';
    } else {
      title = 'Untitled';
      noteText = '';
      isMarkdownActive = false;
      isEditorModeActive = false;
      titleAutoDerive = true;
    }
    lastNoteId = noteId;
  }

  const deriveTitleFromContent = (content: string, isMarkdown: boolean): string => {
    const firstLine = content.split('\n').find((line) => line.trim().length > 0)?.trim() ?? '';
    const cleaned = isMarkdown ? firstLine.replace(/^#{1,6}\s+/, '') : firstLine;
    if (!cleaned) return 'Untitled';
    return cleaned.length > 80 ? cleaned.slice(0, 80) : cleaned;
  };

  const saveActiveNote = async () => {
    if (noteId == null) return;
    // Locked notes only ever reach here via a checkbox toggle (see
    // onReadOnlyChecked in MarkdownEditor.svelte) - route through the
    // narrow exception that persists just the content, rather than the
    // general save, which rejects content changes on a locked note.
    const saved = isLockedActive
      ? await notesStore.saveChecklistToggle(noteId, noteText)
      : await notesStore.saveNote({ id: noteId, title, content: noteText });
    notesStore.applyUpdatedNote(saved);
    status.set('Saved');
  };

  const scheduleSave = () => {
    status.set('Saving…');
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      void saveActiveNote();
    }, 250);
  };

  // Not guarded by isLockedActive: every OTHER path into this (typing in
  // the plain textarea, insertAtCursor, plain-text undo/redo) is already
  // blocked upstream while locked (native readonly / explicit checks), so
  // in practice this only ever runs locked via the Markdown editor's
  // checkbox-toggle exception below - which is exactly the one edit a
  // locked note should still save.
  const handleEditorInput = () => {
    if (titleAutoDerive) {
      title = deriveTitleFromContent(noteText, isMarkdownActive);
    }
    scheduleSave();
  };

  // Also fires for a checkbox toggle in a locked note (see
  // onReadOnlyChecked in MarkdownEditor.svelte) - a locked note's text is
  // read-only, but ticking a finished checklist's items is exactly the
  // kind of edit locking is meant to still allow, and it should save and
  // bump updated_at the same as any other edit.
  const handleMarkdownEditorUpdate = (markdown: string) => {
    noteText = markdown;
    handleEditorInput();
  };

  const handlePlainEditorUpdate = (text: string) => {
    noteText = text;
    handleEditorInput();
  };

  const handleTitleInput = () => {
    if (isLockedActive) return;
    titleAutoDerive = false;
    scheduleSave();
  };

  const handleLanguageChange = (language: string) => {
    if (noteId == null) return;
    void notesStore.saveNote({ id: noteId, language }).then((saved) => {
      notesStore.applyUpdatedNote(saved);
    });
  };

  // Called by App.svelte's commitRename (sidebar double-click rename) when
  // the renamed note happens to be the one this pane is currently showing -
  // the title *input*'s own typing already updates `title` directly
  // (handleTitleInput above), this covers the OTHER path a title can
  // change: an external rename that didn't go through this pane's own
  // input at all.
  export function syncRenamedTitle(id: number, newTitle: string) {
    if (id === noteId) {
      title = newTitle;
      titleAutoDerive = false;
    }
  }

  export function focus() {
    activeTextEditorRef()?.focus();
  }

  export function openSearch() {
    editorModeEditorRef?.openSearch();
  }

  export function toggleMarkdown() {
    if (noteId == null || isLockedActive) return;
    const next = !isMarkdownActive;
    isMarkdownActive = next;
    // Switching modes swaps the textarea/MarkdownEditor DOM out from under
    // whichever one was focused - wait for that swap to render, then focus
    // whichever editor is now showing so typing can continue immediately.
    // While editor mode is on, neither actually (re)mounts - it stays the
    // one editor regardless of this flag - so there's no DOM swap to chase.
    if (!isEditorModeActive) {
      void tick().then(() => {
        if (next) {
          markdownEditorRef?.focus();
        } else {
          plainEditorRef?.focus();
        }
      });
    }
    void notesStore.saveNote({ id: noteId, isMarkdown: next }).then((saved) => {
      notesStore.applyUpdatedNote(saved);
    });
  }

  export async function toggleEditorMode() {
    if (noteId == null) return;
    const saved = await notesStore.toggleEditorMode(noteId);
    if (!saved) return;
    isEditorModeActive = saved.isEditorMode;
    // Same DOM-swap-loses-focus issue as toggleMarkdown - flipping this
    // flag mounts a different editor component entirely (EditorModeEditor
    // vs. Markdown/PlainTextEditor), so whatever was focused is gone; wait
    // for the swap to render, then focus whichever editor is now showing.
    await tick();
    activeTextEditorRef()?.focus();
  }

  export function insertAtCursor(text: string) {
    if (isLockedActive) return;
    activeTextEditorRef()?.insertAtCursor(text);
  }

  // Always available regardless of isEditorModeActive (Alt+F, and the
  // ActionToolbar button) - each editor's own format() resolves the note's
  // persisted language (falling back to auto-detection) and reformats just
  // the current selection if there is one, otherwise the whole note. The
  // resulting edit flows back through that editor's normal onUpdate wiring
  // (same as typing), so it autosaves the same way any other edit does.
  // Editor-mode only: PlainTextEditor/MarkdownEditor don't implement
  // format() (and Alt+F/the toolbar button are hidden outside editor mode
  // to match), since Format only makes sense against the syntax-highlighted
  // CodeMirror view.
  export async function format() {
    if (noteId == null || isLockedActive || !isEditorModeActive) return;
    try {
      await editorModeEditorRef?.format();
      status.set('Formatted');
    } catch (err) {
      status.set(err instanceof Error ? err.message : 'Format failed');
    }
  }

  export function openLinkAtCursor() {
    if (!isEditorModeActive && isMarkdownActive) {
      const href = markdownEditorRef?.getLinkHrefAtCursor();
      if (href) onOpenLink(href);
    }
  }

  // Navigates to the note matching `rawTitle` (case-insensitive, trimmed -
  // same rule wikiLinks.ts's resolveWikiLinkTitle uses), or creates one
  // with that exact title at the root level if none exists yet - the
  // "click a red link to create the page" wiki convention. Root level (not
  // the sidebar's active parent) is a predictable landing spot regardless
  // of whatever the sidebar happens to be scrolled/expanded to.
  const handleOpenWikiLink = async (rawTitle: string) => {
    const needle = rawTitle.trim().toLowerCase();
    const existing = needle ? $notes.find((n) => n.title.trim().toLowerCase() === needle) : undefined;
    if (existing) {
      onNavigate(existing.id);
      return;
    }
    const created = await notesStore.createNoteIn(null, rawTitle.trim() || 'Untitled');
    onNavigate(created.id);
    status.set(`Created note "${created.title}"`);
  };

  // Right-click menu for this pane's title bar / editor content. Handles
  // link clicks via a real native "click" listener in the capture phase,
  // rather than Tiptap/ProseMirror's editorProps.handleClick.
  const openEditorMenu = async (event: MouseEvent) => {
    if (noteId == null) return;
    const id = noteId;
    const x = event.clientX;
    const y = event.clientY;
    // Plugins (contextMenu.registerEditorItem) get the same context object
    // this menu itself uses for the link check below - only the Markdown
    // editor can compute one (links/task items only exist there), so a
    // plain-text note gets a minimal context with isMarkdown: false.
    const pluginContext: EditorContext = !isEditorModeActive && isMarkdownActive && markdownEditorRef
      ? markdownEditorRef.getContext(event)
      : { noteId: id, isMarkdown: false, taskItem: null, linkHref: null, selectionText: '' };
    const linkHref = pluginContext.linkHref;
    const pluginMenuItems: ContextMenuItem[] = getEditorContextMenuItemsFor(pluginContext).map((item) => ({
      label: item.label,
      action: () => item.action(pluginContext),
    }));

    // Text-only: Copy/Cut/Paste here act purely on selected text and the OS
    // clipboard, like a normal text editor's right-click menu - never on
    // whole-note move/duplicate (that's what right-clicking the note itself
    // in the sidebar, and drag-and-drop, are for). Disabled rather than
    // falling back to a note-level operation when there's nothing to act
    // on, so this menu never has a surprise side effect on the sidebar tree.
    const selectedText = activeTextEditorRef()?.getSelectedText() ?? '';
    const copySelection = () => void writeClipboardText(selectedText);
    const cutSelection = () => {
      // Locked notes can't have their content edited - fall back to a
      // non-destructive copy rather than silently failing to delete.
      if (isLockedActive) {
        void writeClipboardText(selectedText);
        return;
      }
      const cutText = activeTextEditorRef()?.cutSelection() ?? '';
      if (cutText) void writeClipboardText(cutText);
    };
    const osClipboardText = (await readText().catch(() => null)) ?? '';
    const pasteSelection = () => insertAtCursor(osClipboardText);

    contextMenu = {
      x,
      y,
      items: [
        ...(linkHref
          ? [
              { label: 'Open link', action: () => void onOpenLink(linkHref) },
              { label: 'Copy link address', action: () => void writeClipboardText(linkHref) },
              { label: '', separator: true },
            ]
          : []),
        { label: 'Copy', disabled: !selectedText, action: copySelection },
        { label: 'Cut', disabled: !selectedText, action: cutSelection },
        { label: 'Paste', disabled: !osClipboardText, action: pasteSelection },
        { label: '', separator: true },
        { label: isLockedActive ? 'Unlock' : 'Lock', action: () => void notesStore.toggleLock(id) },
        ...(pluginMenuItems.length ? [{ label: '', separator: true }, ...pluginMenuItems] : []),
      ],
    };
  };
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<section class="editor-pane" class:unfocused={!focused} on:mousedown|capture={onFocus}>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <header class="topbar" on:contextmenu|preventDefault={openEditorMenu}>
    <div class="title-block">
      {#if noteId != null}
        <input
          bind:value={title}
          class="title"
          placeholder="Untitled"
          readonly={isLockedActive}
          on:input={handleTitleInput}
        />
      {:else}
        <span class="title title-placeholder">No note selected</span>
      {/if}
    </div>
    <div class="header-meta">
      {#if noteId != null}
        <NoteInfoPopover
          createdAt={currentNoteRecord?.createdAt ?? null}
          updatedAt={currentNoteRecord?.updatedAt ?? null}
          effectiveLanguage={selectedNoteEffectiveLanguage}
          onLanguageChange={handleLanguageChange}
          onError={(msg) => status.set(msg)}
          backlinks={selectedNoteBacklinks}
          onOpenBacklink={(id) => onNavigate(id)}
        />
      {/if}
      {#if isLockedActive}
        <svg class="icon lock-indicator" width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" aria-label="Locked">
          <rect x="3.5" y="7" width="9" height="7" rx="1.2" />
          <path d="M5.5 7V4.5a2.5 2.5 0 0 1 5 0V7" />
        </svg>
      {/if}
    </div>
  </header>

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="editor-content" on:contextmenu|preventDefault={openEditorMenu}>
    {#if noteId == null}
      <!-- Split view opens the second pane empty rather than duplicating
           the other one - typing here would have nowhere to save anyway
           (saveActiveNote no-ops with no noteId), so this is a real
           placeholder rather than a fake editable blank note. -->
      <div class="empty-pane">
        <p>Click a note in the sidebar to open it here.</p>
      </div>
    {:else if isEditorModeActive}
      <EditorModeEditor
        bind:this={editorModeEditorRef}
        content={noteText}
        noteId={noteId ?? -1}
        onUpdate={handlePlainEditorUpdate}
        placeholder="Start typing instantly..."
        editable={!isLockedActive}
        showLineNumbers={showLineNumbersActive}
        {vimMode}
        themeMode={theme}
        language={selectedNoteLanguage}
        isMarkdown={isMarkdownActive}
      />
    {:else if isMarkdownActive}
      <MarkdownEditor
        bind:this={markdownEditorRef}
        content={noteText}
        noteId={noteId ?? -1}
        onUpdate={handleMarkdownEditorUpdate}
        {onOpenLink}
        onOpenWikiLink={handleOpenWikiLink}
        noteTitles={$notes.map((n) => ({ id: n.id, title: n.title }))}
        placeholder="Start typing instantly..."
        editable={!isLockedActive}
        {vimMode}
        themeMode={theme}
      />
    {:else}
      <PlainTextEditor
        bind:this={plainEditorRef}
        content={noteText}
        noteId={noteId ?? -1}
        onUpdate={handlePlainEditorUpdate}
        placeholder="Start typing instantly..."
        editable={!isLockedActive}
        showLineNumbers={showLineNumbersActive}
        {vimMode}
      />
    {/if}
  </div>
</section>

{#if contextMenu}
  <ContextMenu x={contextMenu.x} y={contextMenu.y} items={contextMenu.items} onClose={closeContextMenu} />
{/if}

<style>
  .editor-pane {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  /* Only ever applied in split view (a single pane is always "focused") -
     subtle enough to not read as "disabled", just enough to show which
     pane keyboard shortcuts and sidebar clicks currently target. */
  .editor-pane.unfocused {
    opacity: 0.75;
  }

  .editor-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .topbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem 0.9rem;
    border-bottom: 1px solid var(--border);
    gap: 0.75rem;
    /* Matches the ⓘ info button's height (see NoteInfoPopover.svelte's
       .info-btn) - without this, a pane with no note open (no icon to
       prop the row open) renders a shorter header than one that does. */
    min-height: 1.4rem;
  }

  .title-block {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }

  .title {
    display: flex;
    align-items: center;
    /* Explicit rather than relying on font-size/line-height alone - an
       <input> (used when a note is open) and a <span> (the placeholder
       below) don't box the same text identically by default, which made
       the two header states render at very slightly different heights
       even with matching font-size. */
    height: 1.4rem;
    border: 0;
    background: transparent;
    color: inherit;
    font-size: 0.95rem;
    width: 100%;
    outline: none;
  }

  .title-placeholder {
    color: var(--muted);
  }

  .empty-pane {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1rem;
  }

  .empty-pane p {
    margin: 0;
    color: var(--muted);
    font-size: 0.85rem;
  }

  .header-meta {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-shrink: 0;
  }

  .lock-indicator {
    flex-shrink: 0;
    color: var(--muted);
  }
</style>

import { writable } from 'svelte/store';
import type { PluginSettingsService } from '../services/pluginSettingsService';

// What a plugin's context-menu contribution sees, and acts on - computed
// fresh each time the editor's right-click menu opens (see
// MarkdownEditor.svelte's getContext()/App.svelte's openEditorMenu).
export interface EditorContext {
  noteId: number | null;
  isMarkdown: boolean;
  // Non-null only when the right-click landed inside a markdown to-do
  // list row (a TipTap taskItem node).
  taskItem: {
    text: string;
    checked: boolean;
    // Appends " " + label as a link (pointing at url) to the end of this
    // to-do row's text, e.g. for linking back to something created from
    // it. Safe to call later (e.g. after an async action finishes) - it
    // re-checks the row still has the same text before editing, and
    // returns false without changing anything if the document moved on
    // in the meantime (e.g. the row was edited or deleted).
    appendLink: (label: string, url: string) => boolean;
  } | null;
  linkHref: string | null;
  selectionText: string;
}

export interface ContextMenuContribution {
  when: (ctx: EditorContext) => boolean;
  label: string;
  action: (ctx: EditorContext) => void;
}

export interface PluginFormFieldOption {
  label: string;
  value: string;
}

export interface PluginFormField {
  key: string;
  label: string;
  type?: 'text' | 'textarea' | 'select';
  placeholder?: string;
  required?: boolean;
  defaultValue?: string;
  // Required when type is 'select'. Rendered as a themed custom dropdown,
  // not a native <select> - see PluginFormDialog.svelte for why (native
  // <select>/<option> ignore page CSS on Linux/WebKitGTK).
  options?: PluginFormFieldOption[];
}

export interface PluginFormRequest {
  title: string;
  fields: PluginFormField[];
  submitLabel?: string;
  onSubmit: (values: Record<string, string>) => void;
}

export interface PluginMessageRequest {
  title: string;
  message: string;
  // When set, shows a button that opens linkUrl in the system's default
  // browser (via Tauri's opener plugin - not a plain <a href>, so it
  // works the same whether or not FlashPad has webview navigation
  // enabled).
  linkLabel?: string;
  linkUrl?: string;
}

export interface SettingsPanelRequest {
  // Shown as this plugin's own tab label in Settings.
  label: string;
  // Called with an empty container element once the plugin's tab is
  // opened. Full-trust vanilla DOM - build/append whatever you need. May
  // return a cleanup function, called when the tab is navigated away from
  // or the plugin is reloaded.
  render: (container: HTMLElement) => void | (() => void);
}

export interface SettingsPanelContribution extends SettingsPanelRequest {
  pluginId: string;
}

export interface FlashPadPluginAPI {
  contextMenu: {
    // Registers an item that may appear in the editor's right-click menu.
    // `when` is checked every time the menu opens (given the current
    // EditorContext) - return true to include this item that time.
    registerEditorItem(item: ContextMenuContribution): void;
  };
  ui: {
    // Shows a small popup with the given fields, calling onSubmit with
    // the entered values if the user confirms. Rendered by App.svelte's
    // PluginFormDialog, driven by the activePluginForm store below.
    showForm(request: PluginFormRequest): void;
    // Shows a small dismissable message popup, optionally with a button
    // that opens a link (e.g. confirming an action with a link to what it
    // created). Rendered by App.svelte's PluginMessageDialog.
    showMessage(request: PluginMessageRequest): void;
    // Gives this plugin its own tab in Settings. Re-registers every time
    // the plugin is (re)activated - see resetPluginRegistry.
    registerSettingsPanel(request: SettingsPanelRequest): void;
  };
  settings: {
    // Namespaced to this plugin's id - see pluginSettingsService.ts.
    // Plain localStorage, not encrypted - not a place for real secrets.
    get(key: string): unknown;
    set(key: string, value: unknown): Promise<void>;
  };
}

const editorContextMenuItems: ContextMenuContribution[] = [];

export function getEditorContextMenuItemsFor(ctx: EditorContext): ContextMenuContribution[] {
  return editorContextMenuItems.filter((item) => {
    try {
      return item.when(ctx);
    } catch (err) {
      console.error('[flashpad] plugin contextMenu.when() threw', err);
      return false;
    }
  });
}

// Rendered by App.svelte via PluginFormDialog whenever this is non-null.
export const activePluginForm = writable<PluginFormRequest | null>(null);

// Rendered by App.svelte via PluginMessageDialog whenever this is non-null.
export const activePluginMessage = writable<PluginMessageRequest | null>(null);

// Read by SettingsPanel.svelte to render one tab per contribution.
export const pluginSettingsPanels = writable<SettingsPanelContribution[]>([]);

// Clears every plugin's contributions - called before (re)loading plugins,
// since there's no per-plugin unregister yet. A plugin that's still
// enabled re-registers its items when it's reactivated right after.
export function resetPluginRegistry(): void {
  editorContextMenuItems.length = 0;
  activePluginForm.set(null);
  activePluginMessage.set(null);
  pluginSettingsPanels.set([]);
}

export function createPluginApi(pluginId: string, pluginSettingsService: PluginSettingsService): FlashPadPluginAPI {
  return {
    contextMenu: {
      registerEditorItem(item) {
        editorContextMenuItems.push(item);
      },
    },
    ui: {
      showForm(request) {
        activePluginForm.set(request);
      },
      showMessage(request) {
        activePluginMessage.set(request);
      },
      registerSettingsPanel(request) {
        pluginSettingsPanels.update((panels) => [...panels, { pluginId, ...request }]);
      },
    },
    settings: {
      get: (key) => pluginSettingsService.get(pluginId, key),
      set: (key, value) => pluginSettingsService.set(pluginId, key, value),
    },
  };
}

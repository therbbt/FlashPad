# FlashPad plugins

This folder is documentation only - it's not loaded by the app. Plugins
themselves live outside the repo, in a `plugins/` folder inside FlashPad's
app data directory (Settings → Plugins → "Open plugins folder" shows you
exactly where).

## What a plugin is

A folder containing:

- `manifest.json`
  ```json
  {
    "id": "github-issue-from-todo",
    "name": "GitHub Issue from To-Do",
    "version": "1.0.0",
    "description": "Right-click a to-do item to open it as a GitHub issue.",
    "permissions": ["contextMenu", "network"],
    "entry": "index.js"
  }
  ```
  `id` must be unique and stable (it's used as the key for the plugin's
  own settings). `entry` is the JS file FlashPad loads and runs.
- One or more `.js` files. The entry file's default export must have an
  `activate(api)` function:
  ```js
  export default {
    activate(api) {
      // register things using `api` here
    },
  };
  ```

Plugins are loaded at runtime (no rebuild needed) and run as full-trust JS
in the same window as FlashPad itself - there's no sandbox. Only enable
plugins you trust. `permissions` in the manifest are shown to the user
before they enable a plugin, but aren't technically enforced yet.

## The API (`activate(api)`)

- `api.contextMenu.registerEditorItem({ when(ctx), label, action(ctx) })`
  Adds an item to the note editor's right-click menu. `when(ctx)` is
  checked every time the menu opens - return `true` to show the item that
  time. `ctx` (an `EditorContext`) is also passed to `action`:
  ```ts
  interface EditorContext {
    noteId: number | null;
    isMarkdown: boolean;
    // Set only when the right-click landed inside a markdown to-do row.
    taskItem: { text: string; checked: boolean } | null;
    linkHref: string | null;
    selectionText: string;
  }
  ```
- `api.ui.showForm({ title, fields, submitLabel?, onSubmit(values) })`
  Shows a small popup with the given fields and calls `onSubmit` with what
  the user entered, keyed by each field's `key`:
  ```ts
  interface PluginFormField {
    key: string;
    label: string;
    type?: 'text' | 'textarea'; // defaults to 'text'
    placeholder?: string;
    required?: boolean;
    defaultValue?: string;
  }
  ```
- `api.settings.get(key)` / `api.settings.set(key, value)`
  Plain key/value storage namespaced to this plugin's `id` - other plugins
  and FlashPad's own settings can't see or collide with it. This is
  **not** encrypted (same plain localStorage the rest of the app's
  settings use) - don't tell users to put a real secret in here without
  them understanding that.
- Network: just use the global `fetch`. FlashPad's CSP is disabled and
  most public REST APIs (including GitHub's, even for authenticated
  requests) send permissive CORS headers, so no special API or permission
  is needed for that case.

## Worked example: GitHub issue from a to-do row

This is the scenario the API above was designed against - not a plugin
FlashPad ships, just a complete illustration of how you'd build it.

`manifest.json`:
```json
{
  "id": "github-issue-from-todo",
  "name": "GitHub Issue from To-Do",
  "version": "1.0.0",
  "description": "Right-click a to-do item to open it as a GitHub issue.",
  "permissions": ["contextMenu", "network"],
  "entry": "index.js"
}
```

`index.js`:
```js
export default {
  activate(api) {
    api.contextMenu.registerEditorItem({
      // Only show this item when the right-click landed on a to-do row.
      when: (ctx) => ctx.taskItem != null,
      label: 'Create GitHub issue…',
      action: (ctx) => {
        api.ui.showForm({
          title: 'Create GitHub issue',
          fields: [
            { key: 'repo', label: 'Repository (owner/name)', required: true,
              defaultValue: String(api.settings.get('lastRepo') ?? '') },
            { key: 'title', label: 'Title', required: true, defaultValue: ctx.taskItem.text },
            { key: 'body', label: 'Description', type: 'textarea' },
            { key: 'token', label: 'GitHub token', required: true,
              defaultValue: String(api.settings.get('token') ?? '') },
          ],
          submitLabel: 'Create issue',
          onSubmit: async (values) => {
            await api.settings.set('lastRepo', values.repo);
            await api.settings.set('token', values.token);

            const res = await fetch(`https://api.github.com/repos/${values.repo}/issues`, {
              method: 'POST',
              headers: {
                Accept: 'application/vnd.github+json',
                Authorization: `Bearer ${values.token}`,
              },
              body: JSON.stringify({ title: values.title, body: values.body }),
            });
            if (!res.ok) throw new Error(`GitHub API error: ${res.status}`);
          },
        });
      },
    });
  },
};
```

Notes on this example:
- `ctx.taskItem.text` pre-fills the issue title with the to-do row's own
  text, per the original ask this plugin API was built for.
- The token field is a pragmatic v1 choice - it's stored via
  `api.settings`, which is plain (unencrypted) localStorage. A real
  release of this plugin would want the user to understand that, or
  FlashPad would need a proper secret store added later.
- Errors thrown from `onSubmit` aren't caught by FlashPad (yet) - shown
  here for completeness of the example, but a real plugin should surface
  failures to the user itself (e.g. via a second `showForm` call or its
  own toast).

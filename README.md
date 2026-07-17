# FlashPad

FlashPad is a global-hotkey quick-notes desktop app built with Tauri 2 and
Svelte 5. Press a hotkey from anywhere to pop open a lightweight notepad,
jot something down in a tree of notes, and hide it again.

## Features

- **Summon from anywhere** — press a global hotkey (default `Alt+S`,
  configurable in settings) to show/hide the window instantly.
- **Tree-structured notes** — create, rename, delete, duplicate, and move
  notes and subnotes.
- **Markdown editor** — rich text editing powered by Tiptap, with markdown
  shortcuts for headings and formatting.
- **Quick inserts** — dividers, timestamps, and datelines via keyboard
  shortcuts.
- **Note locking** — protect a note from accidental edits.
- **Themeable** — light/dark mode, each with its own independently
  selectable color palette (FlashPad's own light/dark looks, or Catppuccin
  Latte/Frappé/Macchiato/Mocha).
- **Autostart on login**.
- **Local storage** — notes are persisted locally via a SQLite-backed store;
  manage multiple databases, switch between them, and rely on automatic
  local backups with import/export.
- **Automatic updates** — checks GitHub Releases on startup and shows an
  unobtrusive notification (with changelog) when a new version is
  available. Nothing downloads until you confirm.

## Keyboard Shortcuts

| Shortcut | Action |
| --- | --- |
| `Alt+S` (configurable) | Open / restore FlashPad from anywhere |
| `Esc` | Hide the window (still running in the tray) |
| `Alt+N` | Create a new note |
| `Alt+L` | Lock / unlock the current note |
| `Alt+D` | Delete the current note (and its subnotes) |
| `Alt+1` | Insert a divider line |
| `Alt+2` | Insert a timestamp |
| `Alt+3` | Insert a dateline |
| `Enter` | Open the focused note, toggling its subnotes if it has any |
| `↑` / `↓` | Move through the tree or search results |
| `←` / `→` | Collapse / expand the focused note's subnotes |
| Right-click a note | New subnote, rename, duplicate, move, lock, delete |
| Right-click the text | Copy, cut, or paste the note; lock / unlock |
| `Enter` / `Esc` (while renaming) | Confirm / cancel |

## Tech Stack

- [Tauri 2](https://tauri.app/) (Rust) for the desktop shell
- [Svelte 5](https://svelte.dev/) + [Vite](https://vitejs.dev/) + TypeScript for the frontend
- [Tiptap](https://tiptap.dev/) for markdown editing
- Tauri plugins: `autostart`, `global-shortcut`, `window-state`, `dialog`, `updater`, `process`

## Getting Started

### Prerequisites

- [Node.js](https://nodejs.org/)
- [Rust toolchain](https://www.rust-lang.org/tools/install) (required by Tauri)

### Development

```bash
npm install
npm run tauri dev
```

`npm run dev` also works if you only want to preview the frontend in a
browser via Vite, without the Tauri shell.

### Build

```bash
npm run tauri build
```

## Project Structure

- `src/` — Svelte frontend
  - `src/lib/components/` — UI components (notes tree, editor, panels, etc.)
  - `src/lib/services/` — app services (notes, settings, hotkeys, database, autostart)
- `src-tauri/` — Rust/Tauri backend and app configuration

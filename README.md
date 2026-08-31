# FlashPad

FlashPad is a global-hotkey quick-notes desktop app built with Tauri 2 and
Svelte 5. Press a hotkey from anywhere to pop open a lightweight notepad,
jot something down in a tree of notes, and hide it again.

## Features

- **Summon from anywhere** — press a global hotkey (default `Alt+S`,
  configurable in settings) to show/hide the window instantly.
- **Tree-structured notes** — create, rename, delete, duplicate, and move
  notes and subnotes, by drag-and-drop or with `Alt+↑`/`Alt+↓`.
- **Split view** (`Alt+V`, or the toolbar toggle) — open a second note
  side by side with a draggable divider between them. Click into a pane to
  focus it; clicking a note in the sidebar then loads it into whichever
  pane has focus. Both panes are fully independent and support everything
  a single note view does (Editor mode, Format, lock, wiki-links, the
  whole feature set).
- **Markdown editor** — rich text editing powered by Tiptap, with markdown
  shortcuts for headings and formatting. Links open in your default browser
  (Ctrl+Click, a plain click on a locked note, right-click, or Alt+O). Paste
  or drag-and-drop an image (PNG/JPEG/GIF/WebP) to embed it in the note;
  large images are downscaled automatically. Fenced code blocks get real
  per-language syntax highlighting (via `highlight.js`/`lowlight`, the same
  color scheme as Editor mode's CodeMirror view — a note looks the same
  whether or not Editor mode is on), and inline `code` spans are colored as
  a generic command line (there's no way to tell a command apart from any
  other inline code, since markdown gives inline spans no language tag).
  `Alt+C` wraps the current selection in inline code, or starts a fresh
  span with nothing selected.
- **Note linking** — type `[[Note Title]]` in a markdown note to reference
  another note by title; it converts into a clickable link as soon as you
  type the closing `]]`. Click (locked note) or Ctrl/Cmd+click (unlocked
  note) to jump straight to it. A title that doesn't match any note yet
  renders with a dashed, muted style — click it to create that note on the
  spot. The note-info popover (the ⓘ next to the title) gets a **Backlinks**
  section listing every note that links to the one you're viewing.
  Lightweight by design: links resolve by title (not a stable id), so
  renaming a note doesn't rewrite other notes' references to it — they just
  go from resolved to unresolved. No autocomplete; type the title as you
  remember it.
- **Quick inserts** — dividers, timestamps, and datelines via keyboard
  shortcuts.
- **Note locking** — protect a note from accidental edits.
- **Line numbers** — optional per-note gutter, toggled with `Alt+R`
  (Editor mode only). Off by default.
- **Editor mode** — a per-note toggle (`Alt+E`) that swaps a note's view for
  a syntax-highlighted CodeMirror 6 code editor, with its own color scheme,
  search/replace (`Ctrl`/`Cmd+F`), and Tab-to-indent. Language is
  auto-detected from content (JSON, JS/TS, CSS, HTML, XML, YAML, Markdown,
  Python, SQL, shell, and Cisco IOS/Huawei VRP/Nokia SR OS/ADVA network
  device configs) or pinned manually from the note-info popover — a
  markdown note in editor mode is shown as highlighted markdown source
  rather than the rich view. Independent of the Markdown toggle, so any
  note can be prose, code, or both. Network configs get their own
  highlighting too (command keywords, interface/port names, state words
  like `enable`/`in-service`) via a lightweight per-vendor tokenizer, not a
  full grammar. A markdown note's Editor-mode source view also highlights
  fenced code blocks (` ```js `, ` ```bash `, …) using each language's own
  grammar, and colors every inline `` `code` `` span as a generic command
  line (first word as the command, `-x`/`--flag` words, quoted strings,
  and version-like numbers each get their own color) — the same treatment
  the rich Markdown view's inline code gets, for the same reason: there's
  no way to tell a command apart from any other inline code, since markdown
  gives inline spans no language tag.
- **Format** (`Alt+F`, Editor mode only) — real language-aware formatting
  (Prettier, loaded on demand) for JSON, JS/TS, CSS, HTML, YAML, and
  Markdown, including fenced code blocks embedded inside a markdown note;
  structural re-indentation (to each vendor's real convention - 1
  space/level for Cisco IOS and Huawei VRP, 2 for ADVA, 4 for Nokia SR OS)
  for network configs, preserving whatever nesting the source already
  encodes rather than trying to parse command syntax; a safe
  whitespace/indentation tidy-up for everything else, including arbitrary
  text with no formatter — never reflows or reindents content it doesn't
  recognize. Formats just the current selection if there is one, otherwise
  the whole note.
- **Vim mode** — optional modal editing, toggled in Settings → Editor. Off by
  default. Plain text notes get full vim emulation (motions, operators, text
  objects, registers, counts, dot-repeat, `/` search) via CodeMirror 6 and
  `@replit/codemirror-vim`. Markdown notes get a small hand-rolled subset -
  normal/insert modes, `h`/`j`/`k`/`l`, `0`/`$`, `i`/`o`/`O`, `x`, `dd` - since
  no vim-emulation library exists for the rich-text editor. The sidebar also
  supports `j`/`k` to move focus up/down when vim mode is on. The current
  mode (NORMAL/INSERT/etc.) shows in the footer.
- **Themeable** — light/dark mode, each with its own independently
  selectable color palette (FlashPad's own light/dark looks, or Catppuccin
  Latte/Frappé/Macchiato/Mocha).
- **Autostart on login**.
- **Local storage** — notes are persisted locally via a SQLite-backed store;
  manage multiple databases, switch between them, and rely on automatic
  local backups with import/export. Optionally search across every
  registered database at once, not just the active one. Any individual
  note can also be exported to a plain `.txt` file (its raw content -
  markdown source, plain text, or code, unrendered) via the sidebar's
  right-click menu or the Notes menu.
- **Cloud notebooks** (optional) — sign in and sync notes across devices via
  a Supabase-backed notebook, or share one with a friend using a generated
  invite code (owner/collaborator roles, revocable invites). Fully
  optional: with no Supabase project configured, FlashPad works exactly as
  a local-only app. See [`supabase/README.md`](supabase/README.md) for
  setup. Local and cloud databases sit together in one list in
  Settings → All databases, and cross-database search covers both.
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
| `Alt+M` | Toggle Markdown view |
| `Alt+E` | Toggle Editor mode (syntax highlighting, search/replace, language detection) |
| `Alt+V` | Toggle split view (a second note pane side by side) |
| `Alt+F` | Format the current selection, or the whole note if nothing is selected (Editor mode only) |
| `Alt+R` | Toggle the line-number gutter (Editor mode only) |
| `Ctrl`/`Cmd+F` | Open search/replace (Editor mode only) |
| `Alt+↑` / `Alt+↓` | Move the current note up/down among its siblings |
| `Alt+→` | Nest the current note under its previous sibling |
| `Alt+←` | Move the current note out to its parent's level |
| `Alt+O` | Open the link under the caret (Markdown view) |
| `Alt+B` | Switch to the next database |
| `Alt+1` | Insert a divider line |
| `Alt+2` | Insert a timestamp |
| `Alt+3` | Insert a dateline |
| `Alt+C` | Wrap the selection in inline code (or start typing code with nothing selected) |
| `Alt+T` | Toggle focus between the editor and the notes menu |
| `Enter` | Open the focused note, toggling its subnotes if it has any |
| `↑` / `↓` (or `j` / `k` with vim mode on) | Move through the tree or search results |
| `←` / `→` | Collapse / expand the focused note's subnotes |
| Right-click a note | New subnote, rename, duplicate, move, copy/cut/paste, export to .txt, lock, delete |
| Right-click the text | Copy/cut the selection, paste, lock / unlock |
| `Enter` / `Esc` (while renaming) | Confirm / cancel |

## Tech Stack

- [Tauri 2](https://tauri.app/) (Rust) for the desktop shell
- [Svelte 5](https://svelte.dev/) + [Vite](https://vitejs.dev/) + TypeScript for the frontend
- [Tiptap](https://tiptap.dev/) for markdown editing, with [`highlight.js`](https://highlightjs.org/)/[`lowlight`](https://github.com/wooorm/lowlight) for its fenced-code-block syntax highlighting
- [CodeMirror 6](https://codemirror.net/) + [`@replit/codemirror-vim`](https://github.com/replit/codemirror-vim) for the plain text editor and its vim mode; also powers Editor mode's syntax highlighting, search/replace, and per-language grammars (including [`@codemirror/legacy-modes`](https://github.com/codemirror/legacy-modes) for shell, and hand-rolled tokenizers for network device configs and command-line inline code — see `src/lib/theme/`)
- [Prettier](https://prettier.io/) (loaded on demand) for Format's language-aware formatting
- [Supabase](https://supabase.com/) (optional) for cloud notebooks — auth, Postgres, and row-level security; see [`supabase/README.md`](supabase/README.md)
- Tauri plugins: `autostart`, `global-shortcut`, `window-state`, `dialog`, `updater`, `process`, `clipboard-manager`, `opener`

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

Cloud notebooks are entirely optional and need no setup to run the app —
without a `.env`, FlashPad works exactly as a local-only app. To enable
them, copy `.env.example` to `.env` and follow
[`supabase/README.md`](supabase/README.md).

### Build

```bash
npm run tauri build
```

## Project Structure

- `src/` — Svelte frontend
  - `src/lib/components/` — UI components (notes tree, editors, panels, etc.)
  - `src/lib/services/` — app services (notes, settings, hotkeys, database, cloud/Supabase, autostart)
  - `src/lib/stores/` — shared Svelte stores (notes/tree state, database/search state, cloud/auth state, status bar, vim mode indicator)
  - `src/lib/utils/` — language auto-detection and Format's formatting logic, among others
  - `src/lib/theme/` — palettes, Editor mode's code-editor color scheme, the network-config/command-line tokenizers, and the Markdown view's code-block/inline-code highlighting
- `src-tauri/` — Rust/Tauri backend and app configuration (local SQLite databases)
- `supabase/` — SQL schema, RLS policies, and setup docs for optional cloud notebooks

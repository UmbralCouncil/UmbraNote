# Umbra Note

Umbra Note is a local-first Markdown notebook for Linux. It combines a polished writing surface with ordinary `.md` files that remain usable in Neovim, VS Code, Zed, Obsidian, and Git.

The current application uses TypeScript, SolidJS, CodeMirror 6, and Electron. Its visual direction stays restrained: a `#070707` foundation, layered charcoal surfaces, quiet borders, lavender metadata, and a focused violet accent.

The application icon is stored as `assets/note.png`; WebP assets are intentionally not used.

## Current status

This is the beginning of the TypeScript rebuild. The working foundation includes:

- Native filesystem access through a sandboxed Electron preload bridge
- Note discovery under `~/local/UmbraNote/Notes`
- Create, open, search, rename, save, and automatic save
- Same-directory temporary files and atomic replacement for safer saves
- Titles stored independently in readable `.md.umbra.json` sidecars
- A responsive, collapsible SolidJS sidebar and polished empty state
- CodeMirror cursor movement, selection, clipboard, wrapping, and undo/redo
- Obsidian-style Live Preview inside the editable surface
- Toolbar and keyboard formatting that writes portable Markdown syntax
- Unit tests for selection-based Markdown formatting

Set `UMBRA_NOTE_DIR` to use another notes directory.

## Live Preview

Live Preview is not a separate rendered document. CodeMirror always contains the exact Markdown source that is written to disk. Decorations style headings, emphasis, links, quotes, inline code, and fenced code in place. Markdown punctuation is hidden while the cursor is outside a formatted construct and reappears when that construct is edited.

For example, bold formatting still saves:

```markdown
hello **world**
```

No proprietary rich-text representation is involved.

## Development

Enter the reproducible environment and install dependencies:

```sh
nix develop
ELECTRON_SKIP_BINARY_DOWNLOAD=1 npm install
```

Nix supplies the Electron runtime because the npm post-install binary download is intentionally skipped.

Run the development application:

```sh
npm run dev
```

Build and verify:

```sh
npm run typecheck
npm test
npm run build
npm start
```

Use a disposable directory during development:

```sh
UMBRA_NOTE_DIR="$PWD/demo-notes" npm run dev
```

## Keyboard shortcuts

| Shortcut | Action |
| --- | --- |
| `Ctrl+S` | Save the current note |
| `Ctrl+B` | Bold the selection |
| `Ctrl+I` | Italicize the selection |
| `Ctrl+Z` / `Ctrl+Shift+Z` | Undo / redo |

## Architecture

```text
SolidJS UI
    ↓
CodeMirror editor state + Live Preview decorations
    ↓
Portable Markdown operations
    ↓
Sandboxed Electron IPC bridge
    ↓
Atomic filesystem storage
```

The renderer has no direct Node.js access. Filesystem operations live in `electron/main.ts`, the narrow API is exposed by `electron/preload.ts`, framework-independent Markdown edits live in `src-web/editor/format.ts`, and Live Preview decorations live in `src-web/editor/livePreview.ts`.

## Current limitations

- Image syntax is styled but local images are not displayed inline yet.
- Link activation, rendered task-checkbox widgets, tables, and horizontal-rule widgets are not implemented yet.
- Renaming updates the title sidecar and filename without modifying headings or other Markdown content.
- Packaging into an installable system artifact is not configured yet; development and production builds run through the supplied Nix shell.
- Annotations, collaboration, backlinks, wiki links, synchronization, and CRDT support remain deliberately deferred.

The previous Rust/Floem prototype remains in the repository temporarily for reference while the TypeScript rebuild reaches feature parity. It is not the active application.

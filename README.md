# claude-artifact-editor

WYSIWYG editor for Claude Artifact HTML pages. Open an artifact's `.html` file,
edit the text inline like a Google Doc, save it back. No round-trip through an
AI agent to fix a typo.

Built with [Tauri](https://tauri.app) (Rust + native WebKit/WebView2).
~5 MB binary, no Electron.

## How it works
1. Open any `.html` file (Cmd/Ctrl+O), or paste a `claude.ai/artifact/...` URL and click Fetch
2. The page renders in a native webview with `designMode` on — click anywhere and type
3. Save (Cmd/Ctrl+S) writes clean HTML back to disk

The artifact's styles, scripts, and layout render exactly as they would in a
browser. You're editing the live page, not a markdown proxy.

## Features
- **Inline editing** — the whole page is an editable canvas (`designMode`), with a format bar for bold/italic/headings
- **Import from claude.ai** — paste an artifact URL, a browser window opens for auth, click "Import to Editor" to pull the HTML in
- **Source view** — toggle raw HTML editing for structural changes
- **Save / Save As** — dirty tracking with visual indicator
- **Keyboard shortcuts** — Cmd/Ctrl+O, Cmd/Ctrl+S, Cmd/Ctrl+Shift+S
- **Dark mode** — follows system theme

## Build

Requires Rust and platform-specific deps.

### macOS
    xcode-select --install   # if not already
    cargo install tauri-cli
    npm install
    npx tauri build

### Linux (Debian/Ubuntu)
    sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev
    npm install
    npx tauri build

### Windows
    npm install
    npx tauri build

Binaries land in `src-tauri/target/release/bundle/`.

## Layout
- `src/` — frontend: `index.html`, `main.js` (editor logic), `styles.css`
- `src-tauri/src/lib.rs` — Rust backend: file open/save commands via `tauri-plugin-dialog` + `tauri-plugin-fs`
- `src-tauri/src/main.rs` — entry point
- `src-tauri/capabilities/` — Tauri v2 permission grants (main window + artifact-fetch window)

## Dev
    npm install
    npx tauri dev

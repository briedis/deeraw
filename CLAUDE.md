# Deeraw

A desktop drawing app in Rust for Windows and macOS, inspired by Excalidraw (hand-drawn style whiteboard).

## Approach

Build **very incrementally**: one small, runnable step at a time. Each step should compile, run, and be committed before moving on. Don't add features, crates or abstractions ahead of the step that needs them.

## Environment

- Develop on Windows. The project lives at `C:\dev\deeraw`.
- Toolchain: rustup with the MSVC target (`x86_64-pc-windows-msvc`), which needs the Visual Studio C++ Build Tools.
- Build/run: `cargo run`. Release build: `cargo build --release` → `target\release\deeraw.exe`.
- Web build: needs `rustup target add wasm32-unknown-unknown` and Trunk (`trunk.exe` in `~\.cargo\bin`,
  from its GitHub releases). `trunk serve --release` serves it at http://127.0.0.1:8080; `trunk build --release`
  writes static files to `dist/` (ignored by git). `index.html` holds the `<canvas id="deeraw">` that `main.rs`'s
  wasm `main` starts the app in. The web build has no File menu and no Ctrl+O/Ctrl+S (no blocking file
  dialogs in the browser): those parts are `#[cfg(not(target_arch = "wasm32"))]`.

### First-time setup on a new machine

1. Install Rust from https://rustup.rs and accept the prompt to install the Visual Studio C++ Build Tools.
2. Open a new terminal, `cd C:\dev\deeraw`, run `cargo run`. The Deeraw window opens.

## Docs

Details live in `docs/`; read the relevant file before working in that area, and keep it up to date as features change
(the numbered feature list continues across the feature files).

- `docs/features-canvas.md` — 1 Image canvas, 2 Layers panel, 3 Selection & resize, 5 Pan & zoom, 6 Undo/redo, 7 Canvas background
- `docs/features-tools.md` — 4 Toolbar and every tool (Pencil, Arrow, Line, Shape, Blur, Text, Crop, Screenshot)
- `docs/features-ui.md` — 8 Style panel, 9 Menu bar, 10 Remembered settings
- `docs/features-app.md` — 11 Update check, 12 Preferences, 13 History
- `docs/architecture.md` — code layout, view transform, undo, export, egui/eframe notes, crash reports
- `docs/release.md` — icon, license, wgpu, CI, Microsoft Store, Homebrew, release steps, macOS signing and launcher

"Ctrl" in shortcuts means Cmd on macOS (egui's `Modifiers::COMMAND`).

## Rules

- CHANGELOG.md: user-facing changes per release, newest first. Add a line under `## Unreleased` (create it if
  missing) with each commit that changes what users see or get; internal changes (refactors, docs, CI) stay out.
- Git repo on `master`; commit after each working step.
- Never add a `Co-Authored-By:` trailer (or any other co-author credit) to a commit. Commits are
  authored by the repo owner alone, whoever or whatever wrote the code.

## Planned next steps

Ideas, not commitments (eframe/egui was chosen for quick setup, easy mouse input and 2D painting):

1. Rectangle and ellipse shapes.
2. Save/load a drawing to a file (currently only PNG export).
3. Hand-drawn ("rough") style for shapes, arrows and lines (pencil strokes already have one).
4. SVG export.

Order and scope are open to change; confirm with the user before starting each step.

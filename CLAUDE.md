# Deeraw

A Windows desktop drawing app in Rust, inspired by Excalidraw (hand-drawn style whiteboard).

## Approach

Build **very incrementally**: one small, runnable step at a time. Each step should compile, run, and be committed before moving on. Don't add features, crates or abstractions ahead of the step that needs them.

## Environment

- Develop natively on Windows (not WSL). The project lives at `C:\dev\deeraw`.
- Toolchain: rustup with the MSVC target (`x86_64-pc-windows-msvc`), which needs the Visual Studio C++ Build Tools.
- Build/run: `cargo run`. Release build: `cargo build --release` → `target\release\deeraw.exe`.

### First-time setup on a new machine

1. Install Rust from https://rustup.rs and accept the prompt to install the Visual Studio C++ Build Tools.
2. Open a new terminal, `cd C:\dev\deeraw`, run `cargo run`. Expected output: `Hello from Deeraw!`
3. If git complains about "dubious ownership" (the repo was created from WSL), run:
   `git config --global --add safe.directory C:/dev/deeraw`

## Features

Keep this list up to date as features are built.

1. **Image canvas** — white canvas (1024×768 window).
   - Ctrl+V pastes an image from the clipboard, centered on the mouse (or canvas center).
   - Ctrl+C copies the canvas as a PNG: a screenshot of the canvas cropped to the content's
     bounding box (+10px padding), with selection/toolbar hidden for that frame. Only the
     visible part of the canvas is captured. Clipboard gets PNG + DIB formats.
2. **Layers panel** — resizable left panel listing every object, topmost first, with its
   position, size and kind-specific info (source size, point count, font size). Doubles as a
   debug view. Clicking a row selects it.
3. **Selection & resize** — with the Select tool, click an object to select it (new objects are
   selected automatically); click empty canvas to deselect; drag to move. The selected object gets
   an outline and 8 handles: corners resize with the object's *original* aspect ratio (so they
   undo any stretching; opposite corner stays put), edge midpoints resize only horizontally or
   vertically. The grabbed handle tracks the cursor exactly (rect is recomputed each frame from
   the drag-start rect and the grab offset, not by accumulating deltas). Holding Shift resizes from the center.
   Min size 4px. Delete removes the selected object(s).
   **Multi-select:** Ctrl+click (canvas or layers panel) toggles an object in the selection;
   dragging on empty canvas draws a selection box that selects objects entirely inside it
   (Ctrl adds to the selection). Dragging any selected object moves them all. With several
   selected, each gets an outline and there are no resize handles. Hit-testing uses the bounding box, except
   arrows (see below).
4. **Toolbar** — floating at the bottom of the canvas: Select, Pencil, Arrow, Line, Text.
   - Pencil: drag to draw a freehand stroke; stays in Pencil mode.
   - Arrow: drag from start to end; stays in Arrow mode. Arrows are point-to-point:
     selected arrows show a round handle per endpoint (drag to move that point) and no resize box.
     They're grabbed only within 6pt of the line.
   - Line: like Arrow but without a head (same object kind, `head: false`); stays in Line mode.
   - Text: click to place a text box and type (multi-line). Escape or clicking elsewhere finishes;
     empty text is discarded. Double-click a text object to edit it. Text has only corner handles
     (resizing scales the font). Switches back to Select afterwards.
5. **Pan & zoom** — middle-button drag pans. Mouse wheel zooms around the cursor; Ctrl +/- zoom
   around the canvas center; Ctrl+0 resets. Steps of 20% (zoom = 100% + 20% × level, 20%–500%).
   Current zoom shows in a small box at the bottom-right of the canvas; clicking it resets to 100%. Copy captures at the
   current zoom.
6. **Undo/redo** — Ctrl+Z undoes, Ctrl+Y or Ctrl+Shift+Z redoes (up to 200 steps). Covers every
   change to objects: paste, draw, text, move, resize, arrow points, delete. Not in history:
   selection, tool, pan/zoom.
7. **Canvas background** — picker at the bottom-right (next to the zoom box): transparent
   (shown as a checkerboard), white (default) or black. Copy uses it as the PNG background; with
   transparent, the PNG has real alpha (canvas fill and the window clear color are transparent
   for the capture frame, and the screenshot keeps alpha). Not in undo history.
8. **Style panel** — a compact bar just above the toolbar while Pencil, Arrow, Line or Text is active,
   or while a stroke, arrow, line or text is selected. It shows the current color and size; clicking
   either expands its options upwards (picking one collapses it again). Colors: black, white,
   red, orange, green, blue. Size: line width (2/3/6/10px, for pencil and arrows) or font size
   (S/M/L/XL = 16/28/44/64px; non-preset sizes from resizing show as px).
   With a drawing tool it sets the style of new objects; with a selection it restyles every
   selected object that has that setting (undoable) and also becomes the default. A mixed
   selection of lines and text shows both a line width and a font size button; images are skipped. Arrow heads grow with the line width.

## Current state

- All code in `src/main.rs`; only dependency is eframe 0.37. Edition 2024. `cargo test` covers the
  resize math (`resize_to`).
- Objects (`Object { id, rect, kind }`) have a `rect` in canvas pixels, so a fresh image paste
  displays 1:1 at 100% zoom. View transform: `screen = panel_min + pan + canvas * scale`, with
  `scale = zoom / pixels_per_point` (screen points per canvas pixel); the closures `to_screen_pos`,
  `to_screen` and `to_canvas` in `canvas()` implement it. Sizes in canvas pixels (line width,
  font) are multiplied by `scale` when painting; screen drag deltas are divided by it.
- Object/handle/drawing drags use `dragged_by(PointerButton::Primary)` so middle-drag only pans.
- egui's built-in Ctrl +/- UI zoom is disabled (`zoom_with_keyboard = false`). Stroke points
  are normalized (0..1) to `rect`, so move/resize only change `rect`. Arrow endpoints are stored
  in canvas pixels and `rect` is re-derived from them each frame; use `Object::translate` to move. Text `rect`
  is re-synced to the text layout every frame; resizing text scales `font_px`.
- Undo is snapshot-based (`History`): whenever the canvas is settled (no button held, not
  drawing or editing text) and `objects` differs from the last snapshot, a step is recorded.
  New features get undo for free as long as all their state lives in `objects`. Anything that
  feeds into `Object` equality must be stable across frames (e.g. text rects are measured at a
  fixed scale, not the current zoom), or spurious steps get recorded.
- Copy uses `ctx.request_screenshot` (arrives a frame later) rather than CPU rendering.
- eframe 0.37 API notes: the `App` method is `fn ui(&mut self, ui: &mut egui::Ui, frame)` (not
  `update`); panels take `.show(ui, ...)`; `Id::new` is deprecated in favour of `Id::unique`.
- Clipboard needs no extra crates: egui-winit emits `Event::PasteImage` (when the clipboard has
  no text) and `ctx.copy_image()` writes PNG via arboard.
- Not every Unicode glyph exists in egui's default fonts (e.g. `⬉` renders as a box); check
  toolbar icons visually.
- Pencil/Arrow use a custom bitmap crosshair (`ctx.set_cursor_image`, which is sticky, so it's
  cleared every frame when not needed) because the stock `CursorIcon::Crosshair` disappeared.
- From WSL, run Windows cargo via `powershell.exe` (`$env:USERPROFILE\.cargo\bin\cargo.exe`).
- Git repo on `master`; commit after each working step.

## Planned next steps

Ideas, not commitments (eframe/egui was chosen for quick setup, easy mouse input and 2D painting):

1. Freehand drawing: draw strokes with the mouse on a canvas.
2. Basic shapes: rectangle, ellipse, line/arrow.
3. Selecting and moving shapes.
4. Save/load a drawing to a file.
5. Later: hand-drawn ("rough") rendering style, undo/redo, text, pan/zoom, export to PNG/SVG.

Order and scope are open to change; confirm with the user before starting each step.

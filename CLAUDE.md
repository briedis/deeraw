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

## Features

Keep this list up to date as features are built. "Ctrl" means Cmd on macOS (egui's `Modifiers::COMMAND`).

1. **Image canvas** — white canvas (1024×768 window).
   - Ctrl+V pastes an image from the clipboard, centered on the mouse (or canvas center).
   - Ctrl+S saves the same image as a PNG file: a Save As dialog (rfd) suggests
     `YYYY-MM-DD_HHMMSS.png` (local time) and opens in the last save's folder (per session).
     `.png` is added if no extension is typed; a failed write shows an error dialog.
   - Pasting into an empty canvas fits the window to the image instead: inner size = image + 80pt
     margin on each side + layers panel (if shown), at least 800×600, and the image centered. If that would
     exceed 75% of the screen, it zooms out (20% steps) until it fits. Skipped when maximized or
     fullscreen. The window isn't moved, so a big one can extend past the screen edge.
   - Ctrl+C copies everything (not just what's visible) as a PNG cropped to the content's bounding
     box, trimmed to the pixels that differ from the background (no padding), at 1 canvas pixel per PNG pixel whatever the zoom (scaled down only past
     the GPU's max texture size). No selection or UI in it. Clipboard gets PNG + DIB formats.
     A tooltip-style notice next to the pointer (canvas center if it's elsewhere) confirms it with the
     image's resolution and PNG size (from our own encoding, `encode_png`; the clipboard encodes separately,
     so it's approximate), for 2.5s with a fade (`notice`). Cut shows it too.
   - The window title shows the drawing's size live, "Deeraw (1280 × 720)" (`update_title`), from the
     objects' rects, so it can be a few px larger than the trimmed copy. Just "Deeraw" when empty.
2. **Layers panel** — hidden by default; Ctrl+L toggles it (works anywhere, even while typing). Resizable left panel listing every object, topmost first, with its
   position, size and kind-specific info (source size, point count, font size). Doubles as a
   debug view. Clicking a row selects it. Showing/hiding/resizing it doesn't move the drawing
   on screen (`pan` is compensated when the canvas panel's position changes).
3. **Selection & resize** — with the Select tool, click an object to select it (new objects are
   selected automatically); click empty canvas to deselect; drag to move. The selected object gets
   an outline and 8 handles: corners resize with the object's *original* aspect ratio (so they
   undo any stretching; opposite corner stays put), edge midpoints resize only horizontally or
   vertically. The grabbed handle tracks the cursor exactly (rect is recomputed each frame from
   the drag-start rect and the grab offset, not by accumulating deltas). Holding Shift resizes from the center.
   Min size 4px. Delete or Backspace removes the selected object(s); Ctrl+D duplicates them (copies offset by
   20px, on top, and selected). Ctrl+A selects everything (and switches to Select).
   **Multi-select:** Ctrl+click (canvas or layers panel) toggles an object in the selection;
   dragging on empty canvas draws a selection box that selects objects entirely inside it
   (Ctrl adds to the selection). Dragging any selected object moves them all. With several
   selected, each gets an outline and there are no resize handles. Hit-testing uses the bounding box, except
   arrows (see below).
4. **Toolbar** — floating at the bottom of the canvas: icon buttons (tooltips name them and their key) for Select, Pencil, Arrow, Line, Shape, Blur (a droplet), Text, plus Crop while exactly one image is selected; after a vertical separator, the canvas background (a single swatch that opens the others, like a style panel section) and the zoom level as a plain percentage (fixed width, so the bar keeps its size). The icons are traced from the Deeraw icon set (32×32 grid) and painted as strokes in `widgets.rs` (`Icon`, `icon_button`), so no SVG renderer is needed. The toolbar and the style panel follow the canvas background (`canvas_ui_visuals` in `style.rs`): white with slate icons on a white or transparent canvas, dark gray with light icons on black; the active tool is a blue button with a white icon.
   Shortcuts (no modifiers, not while typing): V Select, P Pencil, A Arrow, L Line, S Shape, B Blur, T Text, C Crop;
   Escape goes back to Select and deselects everything (while cropping, it only ends the crop). Switching tools cancels a half-drawn shape or selection box.
   Button tooltips show the key.
   - Pencil: drag to draw a freehand stroke (it starts on press, so a click leaves a dot); stays in Pencil mode. Two pen styles (style panel):
     plain (uniform width) or hand-drawn, like Excalidraw's freehand: the points are smoothed
     (strongly where close together, i.e. slow jitter; barely where far apart) and drawn as a
     Catmull-Rom spline (`spline`), and the width follows drawing speed (slow → up to 1.5× the line width, fast, from 2500 px/s
     → down to 0.4×). Speed-based pressures are recorded for every stroke, so a stroke can be
     switched between styles afterwards. While drawing, every `PointerMoved` event of the frame
     is recorded (not just the latest position), with times spread by distance (`add_stroke_points`). Hand-drawn strokes are painted as one feathered mesh
     (`paint_hand_drawn`) plus round caps and joins at sharp turns.
   - Arrow: drag from start to end; stays in Arrow mode. Arrows are point-to-point:
     selected arrows show a round handle per endpoint (drag to move that point) and no resize box.
     They're grabbed only within 6pt of the line.
     **Snapping:** while drawing an arrow/line or dragging an endpoint, text and image objects whose
     border is within 40pt of the pointer show a circle 16px outside each side's center (clear of the resize handles) (`anchor`);
     an end within 12pt of a circle snaps to it (filled circle) and attaches (`bindings: [start, end]`,
     a `Binding { id, side }` each). Attached ends follow their object every frame (`follow_bindings`,
     also run just before recording undo so both land in one step). Dragging an endpoint away
     detaches it; moving an arrow without its object detaches it (`detach_selected`); Ctrl+D copies
     re-attach to copied objects or detach. Deleting the object leaves the arrow (its binding dangles
     harmlessly; ids are never reused, and undo re-attaches it).
     **Connecting with Select:** the same circles show around text and images near the pointer; dragging
     from one (crosshair cursor) draws an arrow attached there without picking the Arrow tool, with the
     current arrow style, and selects it (`connect_ui` in `snap.rs`, registered after the objects so the
     press doesn't move them, before the selection handles so those still win).
   - Line: like Arrow but without a head (same object kind, `heads: None`); stays in Line mode.
   - Shape (S): drag corner to corner to draw a rectangle, ellipse or diamond (`Kind::Shape`, picked in
     the style panel; the toolbar button shows the current one); Shift makes it a square/circle. Outline
     only by default (`fill: None`, shared with text boxes' fill); an unfilled shape is grabbed only
     near its outline (`distance_to_outline`) unless selected, so things inside it stay clickable.
     Resizes freely; arrows attach to shapes like to text and images. Stays in Shape mode.
   - Blur (B): drag a box over part of an image to pixelate it, for hiding things in screenshots
     (`Kind::Blur`, `app/blur.rs`). The drag may start outside: the box attaches to the first image it
     touches (topmost if several) and is cut to that image's edges; one that never touches an image
     shows a notice. Pixelated, not gaussian, so it's safe: on paste each image also gets `mosaics`
     textures (`mosaic.rs`: every block averaged into one pixel, nearest sampling), and a blur paints
     one over its area (`paint_mosaic`), so the original pixels are never drawn there, on screen or in
     exports. Three strengths (`BlurStrength`, three checkered buttons right in the style panel bar, 4×4, 3×3
     and 2×2, also for selected blurs): light (12px blocks, the default), medium (24px) and strong (48px); the stronger mosaics are
     built from the light one. Block sizes are canvas pixels, not the image's: images keep their
     `pixels` (`Pixels`, compared by identity), and `update_mosaics` rebuilds a blurred image's mosaics
     when its scale changes (once the button is released; mid-resize the old blocks scale along), so a
     scaled-up image stays just as blurry. Blocks are aligned to the image's pixels. A blur stores its `area` relative to the whole
     uncropped image, and `follow_blurs` re-derives its `rect` every frame, so it moves, resizes and crops
     with the image; moving/resizing the blur itself updates `area` (`update_blur_areas`). It's inserted
     right above its image, so annotations stay on top. Deleting or cutting an image takes its blurs
     along; Ctrl+D moves copied blurs onto the copied image. Blurs resize freely. Picking a blur with
     Select (click or drag) switches to the Blur tool, which also selects, moves and resizes blurs (only
     blurs; clicking elsewhere deselects) and restyles the selected ones. While cropping, the image shows unblurred (on screen only).
   - Text: two modes. Click to place auto-width text: one line per Enter; it has only corner
     handles and resizing scales the font. Drag a box (≥20px wide) for a text box (`TextFrame`):
     lines break at the box width and the text is aligned inside it (9 alignments, picked in the
     style panel) over a background (transparent by default, or a palette color, also in the style
     panel); the box grows taller if the text overflows. All 8 handles resize the box freely
     and never change the font. The box is outlined while editing. Escape or clicking elsewhere finishes; empty text is discarded.
     Double-click a text object to edit it; with Select, double-clicking empty space starts a text there (switching to Text). The Text tool stays active while typing (the style panel restyles that text too); finishing (Escape or clicking elsewhere) switches back to Select (`end_editing`). A press on the canvas ends typing before the tool sees it, so clicking away doesn't start another text.
   - Crop: not a tool but a mode for one image. Enter it with the Crop button or C (shown/working only
     while exactly one image is selected) or by double-clicking an image. The whole source image shows dimmed and
     outlined, the kept part bright with 8 handles that drag its edges/corners freely within the
     source; dragging inside it slides the kept part over the source. Enter, Escape, C, the Crop
     button, clicking elsewhere or switching tools finishes. Non-destructive: images store `uv`
     (the kept part of the texture, 0..1), so recropping can widen it again. After cropping,
     corner resizing keeps the cropped part's undistorted proportions. One undo step per crop session.
5. **Pan & zoom** — middle-button drag pans, and so does left-drag while holding Space (not while
   typing; open-hand cursor while held, closed hand while panning). A blocking widget over the canvas keeps that drag from objects
   and tools. Mouse wheel zooms around the cursor; Ctrl +/- zoom
   around the canvas center; Ctrl+0 resets. Steps of 20% (zoom = 100% + 20% × level, 20%–500%).
   Current zoom shows as a percentage at the right end of the toolbar; clicking it resets to 100%.
6. **Undo/redo** — Ctrl+Z undoes, Ctrl+Y or Ctrl+Shift+Z redoes (up to 200 steps). Covers every
   change to objects: paste, draw, text, move, resize, arrow points, delete. Not in history:
   selection, tool, pan/zoom.
7. **Canvas background** — swatch in the toolbar (next to the zoom level): transparent
   (shown as a checkerboard), white (default) or black. Copy uses it as the PNG background; with
   transparent, the PNG has real alpha (the offscreen render clears to transparent).
   Each background has a default ink: black on white/transparent, white on black. Switching the
   background while the drawing color is the old ink switches it to the new one (other colors
   are kept; existing objects are not recolored). Not in undo history.
8. **Style panel** — a compact bar just above the toolbar while Pencil, Arrow, Line, Shape or Text is active,
   or while a stroke, arrow, line, shape or text is selected. It shows the current color and size; clicking
   either expands its options upwards, centered above that button (picking one collapses it again). The panels stack: toolbar, then the style panel (centered above the current tool's button, or with Select above the tool that draws the first selected object, `tool_anchor`), then the open options (`style_anchor` is the open button's center, set by `style_toggle`). Colors: black, white,
   red, orange, green, blue. Pen style (pencil and strokes): plain or hand-drawn. Arrow heads (arrows, not lines; `ArrowHeads`/`Head` in `object.rs`): a start button (pointing left) and an end button (pointing right), each previewing that end and opening its options: none, open lines or solid triangle (default: none at the start, solid at the end). Size: line width (2/3/6/10px, for pencil and arrows; its button shows the stroke-width icon, the px in its tooltip) or font size
   (S/M/L/XL = 16/28/44/64px; non-preset sizes from resizing show as px); text boxes and the Text tool
   also get an alignment button (3×3 grid); text boxes, shapes and the Text and Shape tools get a fill button
   (transparent or a palette color). Shapes also get a shape button (rectangle, ellipse, diamond), first in the bar.
   The Blur tool and selected blurs get only the three strength buttons, in the bar itself (no section).
   With a drawing tool it sets the style of new objects; with a selection it restyles every
   selected object that has that setting (undoable) and also becomes the default. A mixed
   selection of lines and text shows both a line width and a font size button; images are skipped. Arrow heads grow with the line width; an open head is one path so its point stays sharp when thick, and the line stops inside a solid head so its square end doesn't show.

9. **Menu bar** — top of the window (`menu_bar`).
   - File: Open image… (Ctrl+O; png/jpg/bmp/gif/webp via the `image` crate, placed like a paste at the
     canvas center, fitting the window if the canvas is empty), Save as… (Ctrl+S), Exit.
   - Edit: Undo, Redo, Cut, Copy, Paste, Delete, Select all, Duplicate, with shortcuts shown. Cut/Copy/Paste
     send `ViewportCommand::Request*`, so they produce the same clipboard events as the keys (and act on
     text being edited). A menu Paste goes to the canvas center, not under the pointer.
   - Cut (Ctrl+X) copies only the selected objects as a PNG and deletes them; Copy still copies
     everything. Exports carry their own snapshot of objects (`Export::new`) for that reason.
   - Help: About Deeraw — logo, name, tagline (`description` in Cargo.toml), version and author
     (`authors`), read with `env!`. Escape or × closes it. It also links the license.
10. **Remembered settings** — the style (color, sizes, pen, arrow heads, alignment, fill), the canvas
    background and the last file folder survive a relaunch, via eframe's `persistence` feature
    (`Settings` and `App::save` in `app/mod.rs`; `app.ron` in `%APPDATA%\deeraw\data`, localStorage on
    the web), as do the window size and position. Saved on exit and every 30 seconds. `#[serde(default)]`
    keeps older settings files loading as fields are added; unreadable ones are ignored. The drawing
    isn't kept, and the app always starts with Select.

## Current state

- Code layout (`src/`): `main.rs` (native window / web canvas setup, `mod`s), `object.rs` (`Object`, `Kind`, `TextFrame`),
  `style.rs` (`Style`, palette/sizes, `Background`), `history.rs` (undo), `geometry.rs` (resize/crop/zoom math),
  `stroke.rs` (pencil points, pressure, hand-drawn painting), `paint.rs` (painting objects), `mosaic.rs` (pixelation), `export.rs`
  (offscreen render, PNG), `widgets.rs` (style panel/menu buttons), `cursor.rs`. `app/mod.rs` holds `DeerawApp`,
  its state types and object operations; its `impl` continues in `app/canvas.rs` (input, tools, painting,
  text editing), `app/selection.rs` (handles, crop), `app/snap.rs` (arrow snapping/attaching), `app/blur.rs` (Blur tool), `app/panels.rs` (layers, toolbar, style panel),
  `app/menu.rs` (menu bar, About) and `app/files.rs` (paste/open, fit window, save, export).
  Constants live in the module that uses them. Methods called across `app/` files are `pub(super)`.
  Tests sit in each module's `mod tests`. Dependencies: eframe 0.37, image (decodes opened files), rfd (file/message dialogs), png (already
  in eframe's tree; writes saves), chrono (local time for file names). Edition 2024. Dev builds optimize
  dependencies (`[profile.dev.package."*"]`), since unoptimized egui/wgpu made window dragging laggy. `cargo test` covers the
  geometry (resize, crop, zoom), stroke pressure and spline, undo/redo, selection, arrow snapping and PNG writing.
- App icon: `assets/deer.svg` is the source (dark variant: #242424 rounded square, white deer drawn as
  round-capped strokes). `python3 assets/make_icons.py` (ImageMagick + Pillow) renders it
  to `assets/icon.ico` (16–256px), embedded in the exe by `build.rs` (winresource, needs the Windows
  SDK's rc.exe), and `assets/icon.png` (256px), the window/taskbar icon (`with_icon`).
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
- Copy renders offscreen with eframe's own `egui_wgpu::Renderer` (`frame.wgpu_render_state()`), so it
  shares the font atlas and image textures: objects are painted into a throwaway layer whose shapes
  are taken back out (`export_shapes`), tessellated and rendered to a texture, then read back
  (`render_offscreen`). It takes two frames because new glyphs only reach the GPU atlas at the end
  of a frame.
- eframe 0.37 API notes: the `App` method is `fn ui(&mut self, ui: &mut egui::Ui, frame)` (not
  `update`); panels take `.show(ui, ...)`; `Id::new` is deprecated in favour of `Id::unique`.
- An `Area` with a pivot other than LEFT_TOP is placed using last frame's size, so it's misplaced for a frame when its content resizes; `redo_frame_if_resized` (panels.rs) discards that frame (`ctx.request_discard`).
- Clipboard needs no extra crates: egui-winit emits `Event::PasteImage` (when the clipboard has
  no text) and `ctx.copy_image()` writes PNG via arboard.
- Not every Unicode glyph exists in egui's default fonts (e.g. `⬉` renders as a box); check
  toolbar icons visually.
- Pencil/Arrow/Line use a custom bitmap crosshair and Text a custom I-beam (`outlined_cursor`) (`ctx.set_cursor_image`, which is sticky, so it's
  cleared every frame when not needed) because the stock `CursorIcon::Crosshair` disappeared. Panning shows
  pixel-art hands (`pixel_art_cursor`): on Windows `CursorIcon::Grab`/`Grabbing` are the four-way move arrows.
- License: PolyForm Shield 1.0.0 (`LICENSE.md`, with a `Required Notice:` copyright line that
  must stay; `license` in Cargo.toml). Any use is allowed, commercial included, except providing a
  product that competes with Deeraw. Source-available, not OSI open source.
- wgpu is set up in `wgpu_options` (`main.rs`). The surface uses `SurfaceConfig::LOW_LATENCY` (1 queued frame);
  `HIGH_THROUGHPUT` (2 frames) made window resizing jittery on Windows (the presented frame lagged the
  window size and was stretched to fit), and window dragging is smooth with 1 frame too.
  `InstanceFlags::VALIDATION_INDIRECT_CALL` is cleared because it stopped the app from starting on
  older GPUs: wgpu validates indirect draws with an internal compute pipeline built while the device
  is created, Metal's shader compiler fails to build it on an Intel Iris Pro ("Compiler encountered
  an internal error"), and `request_device` then fails with `Device(Lost)` before any window appears.
  Nothing here draws indirectly. Symptoms of it coming back: the app exits at startup with
  `Error: Wgpu(RequestDeviceError(... Device(Lost)))` and no window.
- Release builds use `windows_subsystem = "windows"` (no console window); debug builds keep the console.
- CI: `.github/workflows/build.yml` builds Windows (x64 zip) and macOS (universal `Deeraw.app` + `Deeraw Launcher.app` zip) on every push/PR; a `v*` tag publishes a release.
- macOS menu bar launcher (`macos-launcher/`): a separate Swift app (`main.swift`, `LSUIElement`, no Dock icon) that
  shows the deer in the menu bar; left click opens `/Applications/Deeraw.app`, right click shows Open/Quit. It
  registers itself as a login item (`SMAppService`, needs it in `/Applications`). `build.sh [out_dir] [icns]`
  builds it universal (default: installs to `/Applications`); the menu bar PNGs are committed, rendered from
  `menubar-deer.svg` (the deer from `assets/deer.svg`, black, thicker strokes, no background).
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

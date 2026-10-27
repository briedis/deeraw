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
   - The app starts with an intro on the canvas (`app/intro.rs`): the deer (painted as strokes from `assets/deer.svg`'s
     paths, `paint_deer`, so it takes the background's colors), "Add an image to mark it up", rows for Paste (like
     Edit > Paste), Take a screenshot (macOS/Windows) and Open an image… (native) with their shortcuts, and a
     hint that drawing works right away. Only the rows take input. It goes for good (`intro_dismissed`) once
     there's an object or a tool other than Select is picked; emptying the canvas doesn't bring it back.
   - Ctrl+V pastes an image from the clipboard, centered on the mouse (or canvas center).
     Images with a side over the GPU's texture limit (`max_texture_side`, usually 8192) are scaled
     down to fit (`fit_texture_limit` in files.rs), with a notice; egui panics on bigger textures.
   - Pasting text outside a text box (egui reports text, not an image, when the clipboard has both) places it as inline text in
     the Text tool's style (font size, text color, alignment, background), centered like an image paste, and
     selects it (`place_text`). Markdown in it is formatted. `--paste` places only images.
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
   The image pasted or opened onto an empty canvas (`first_image`) also resizes from its center, until
   it's moved; later images, and that one once moved, resize from the opposite corner/side.
   **Alignment snapping:** moving a selection of only images, text and shapes (its bounding box; blurs,
   strokes and arrows never snap) snaps its center to line up with
   an unselected image, text or shape's center within `ALIGN_DISTANCE` (6pt), per axis (centers only,
   not edges), and shows red guide lines through the lined-up centers (spanning both objects). Hold
   Alt (Option) to move freely. A move keeps its unsnapped total (`move_drag`), so it snaps and
   unsnaps without drift; the math is `align_offset`/`align_guides` in geometry.rs. An object whose
   center lines up with another's when a resize starts resizes from the center on that axis
   (`ResizeDrag::centered`, `centered_on`), so it stays lined up; resize handles don't snap.
   Only objects at least about as big count (`align_candidates`: the moved one's area at most 1.25×
   theirs), so a big object isn't pulled onto small ones, while small ones still line up with it.
   **Riding along:** moving an image moves what's on it (`riders`, decided when the move starts; `riders` field):
   unselected objects above it with at least half their area inside it (`mostly_inside`; a line's center), when it's
   the topmost image they're on. Nested, so an image on an image takes its own along. Blurs follow their image anyway
   and attached arrows their objects, so they're left out. Alt moves just the selection. Riders don't count as
   alignment targets. No stored link: moving an annotation off an image unlinks it. Resizing or deleting the image
   leaves them be.
   Min size 4px. Delete or Backspace removes the selected object(s); Ctrl+D duplicates them (copies offset by
   20px, on top, and selected). Ctrl+A selects everything (and switches to Select).
   **Multi-select:** Ctrl+click (canvas or layers panel) toggles an object in the selection;
   dragging on empty canvas draws a selection box that selects objects entirely inside it
   (Ctrl adds to the selection). Dragging any selected object moves them all. With several
   selected, each gets an outline and there are no resize handles. Hit-testing uses the bounding box, except
   arrows (see below).
4. **Toolbar** — floating at the bottom of the canvas: icon buttons (tooltips name them and their key): first (macOS and Windows, then a separator) Screenshot, then Select, Pencil, Arrow (it shows the line icon in line mode), Shape, Blur (a droplet), Text, plus Crop while exactly one image is selected; after a vertical separator, Undo and Redo (U-turn arrows,
   faded while there's nothing to undo/redo; the Edit menu items are disabled then too); after another, the canvas background (a single swatch that opens the others, like a style panel section) and the zoom level as a plain percentage (fixed width, so the bar keeps its size). The icons are traced from the Deeraw icon set (32×32 grid) and painted as strokes in `widgets.rs` (`Icon`, `icon_button`), so no SVG renderer is needed. The toolbar and the style panel follow the canvas background (`canvas_ui_visuals` in `style.rs`): white with slate icons on a white or transparent canvas, dark gray with light icons on black; the active tool is a blue button with a white icon.
   - Screenshot (macOS; `app/screenshot.rs`): orders the window out and runs `screencapture -i` on a thread (drag an
     area, Space for a window, Escape cancels); the capture is placed like an opened image. The window is hidden and
     shown through AppKit (`orderOut`, then `makeKeyAndOrderFront` on the main queue via dispatch2), not
     `ViewportCommand::Visible`: eframe runs no frames for a hidden window, so it could never show itself again; and
     hiding the app instead (Cmd+H style) loses activation, which macOS won't give back, so it returned behind other apps.
   - Screenshot (Windows; same file): minimizes the window and opens Snipping Tool's area selection
     (`ms-screenclip:`, as Win+Shift+S). Its only result is the clipboard, so a thread polls
     `GetClipboardSequenceNumber` and reads the image with arboard. The selection takes the foreground while
     open, so once another window has it again with no new image (after `CLIPBOARD_WAIT`), it was cancelled;
     restoring Deeraw by hand also cancels, and so does the selection not opening within `OPEN_TIMEOUT`. The thread restores
     the window through Win32 (`bring_back`), attaching to the foreground window's input thread so
     `SetForegroundWindow` isn't refused by focus-stealing protection (without it, Deeraw came back behind).
   Shortcuts (no modifiers, not while typing): V Select, P Pencil, A Arrow, L Arrow in line mode, S Shape, B Blur, T Text, C Crop;
   Escape goes back to Select and deselects everything (while cropping, it only ends the crop). Switching tools cancels a half-drawn shape or selection box.
   Button tooltips show the key.
   - Pencil: drag to draw a freehand stroke (it starts on press, so a click leaves a dot); stays in Pencil mode. Two pen styles (style panel):
     plain (uniform width) or hand-drawn, like Excalidraw's freehand. Both are smoothed when painted
     (`smooth_stroke`): points closer than 4 physical pixels to the last kept one are dropped (mouse
     pixel steps make slow lines staircase), the rest pulled together (strongly where close, i.e.
     slow jitter; barely where far apart) and drawn as a Catmull-Rom spline (`spline`). In the
     hand-drawn style the width follows drawing speed (slow → up to 1.5× the line width, fast, from 1200 px/s
     → down to 0.4×). Speed-based pressures are recorded for every stroke, so a stroke can be
     switched between styles afterwards. While drawing, every `PointerMoved` event of the frame
     is recorded (not just the latest position), with times spread by distance (`add_stroke_points`). Hand-drawn strokes are painted as one feathered mesh
     (`paint_hand_drawn`) plus round caps and joins at sharp turns.
   - Arrow: drag from start to end; stays in Arrow mode. Arrows are point-to-point:
     selected arrows show a round handle per endpoint (drag to move that point) and no resize box.
     They're grabbed only within 6pt of the line.
     **Snapping:** while drawing an arrow/line or dragging an endpoint, text and image objects whose
     border is within 40pt of the pointer show a circle 16px outside each side's center (clear of the resize handles) (`anchor`);
     with the Arrow tool every object shows them, also before a drag (the one under the pointer filled);
     an end within 12pt of a circle snaps to it (filled circle) and attaches (`bindings: [start, end]`,
     a `Binding { id, side }` each). Attached ends follow their object every frame; an arrow attached at only one end moves
     along whole (its free end shifts as much), one attached at both ends stretches (`follow_bindings`,
     also run just before recording undo so both land in one step). Dragging an endpoint away
     detaches it; moving an arrow without its object detaches it (`detach_selected`); Ctrl+D copies
     re-attach to copied objects or detach. Deleting the object leaves the arrow (its binding dangles
     harmlessly; ids are never reused, and undo re-attaches it).
     **Connecting with Select:** the same circles show around text and images near the pointer; dragging
     from one (crosshair cursor) draws an arrow attached there without picking the Arrow tool, with the
     current arrow style, and selects it (`connect_ui` in `snap.rs`, registered after the objects so the
     press doesn't move them, before the selection handles so those still win).
   - Line: not a tool of its own but the Arrow tool's line mode (`Style::line`, picked with L or the style panel's
     Arrow/Line button, first in the bar, like Shape's): draws an arrow without heads (`heads: None`). Picking Line or
     Arrow for selected objects removes their heads or gives them the default ones. Connecting with Select always draws arrows.
   - Angle snapping (arrows and lines): a free end (not attached), while drawing or dragging its
     handle, snaps onto the horizontal, vertical or 45° line through the other end when within
     `ANGLE_SNAP_DEGREES` (2°) of it (`snap_angle` in geometry.rs, projected so its length along that line stays).
   - Shape (S): drag corner to corner to draw a rectangle, ellipse or diamond (`Kind::Shape`, picked in
     the style panel; the toolbar button shows the current one); Shift makes it a square/circle. Outline
     only by default (`fill: None`, shared with text boxes' fill); an unfilled shape is grabbed only
     near its outline (`distance_to_outline`) unless selected, so things inside it stay clickable.
     Resizes freely; arrows attach to shapes like to text and images. Stays in Shape mode.
     The line width options for shapes start with **no border** (`border: false`; a struck-through circle).
     A shape always has a border or a fill (`keep_visible`, `keep_shapes_visible` for the style): turning
     the border off on an unfilled shape fills it with its color, and making a borderless shape's fill
     transparent brings the border back. Picking a width turns the border on again.
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
     along; Ctrl+D moves copied blurs onto the copied image. Blurs resize freely. Clicking a blur with
     Select switches to the Blur tool, which also selects, moves and resizes blurs (only
     blurs; clicking elsewhere deselects) and restyles the selected ones. Dragging an unselected blur with Select moves
     the image under it instead (blurs only take clicks then), so images aren't grabbed by their blurs. Switched to that way, clicking
     anything but a blur goes back to Select (selecting what was clicked; `blur_from_select`) until a
     new blur is dragged out; chosen with B or the toolbar, it stays for more blurs. Either way, clicking empty canvas deselects and goes back to Select. While cropping, the image shows unblurred (on screen only).
   - Text: two modes, both with an alignment and a background (`align`, `fill` on `Kind::Text`; picked in the
     style panel; background transparent by default, or a palette color). Click to place auto-width text:
     one line per Enter, the lines aligned left/center/right by the alignment's x; its rect is the text
     plus padding (`text_padding`: 0.2× the font size each side, 0.05× above and below), and the
     background is painted behind each non-empty line only, padded alike, so it stays within the rect
     (`paint_row_fills`); it has only corner
     handles and resizing scales the font. Drag a box (≥20px wide) for a text box (`frame`: its size):
     lines break at the box width and the text is aligned inside it (9 alignments) and the background
     fills the box; the box grows taller if the text overflows. All 8 handles resize the box freely
     and never change the font. The box is outlined while editing. Escape or clicking elsewhere finishes; empty text is discarded.
     Double-click a text object to edit it; with Select, double-clicking empty space starts a text there (switching to Text). The Text tool stays active while typing (the style panel restyles that text too); finishing (Escape or clicking elsewhere) switches back to Select (`end_editing`). A press on the canvas ends typing before the tool sees it, so clicking away doesn't start another text.
     **Markdown** (`markdown.rs`): the text is stored as typed and laid out with basic formatting:
     Slack's markers, one character each: `*bold*`, `_italic_`, `~strike~`, `` `code` `` (monospace on a faint
     background of the text color) within a line; `#`/`##`/`###` headings (2×/1.5×/1.25× the font size, bold) and
     `-`/`*` bullets (shown as `•`) and `1.` items (indented) at the start of one. A marker opens only at a word's
     start before a non-space with a closer later in the line, and closes at a word's end; doubled (`**`) it's
     literal, so `2*3`, `a * b`, snake_case and `~5 min` stay as typed. While editing, the markup shows faded (the
     editor's layout must contain exactly the edited text); otherwise it's hidden, so the rect shrinks when editing
     ends. Ctrl+B / Ctrl+I wrap the selection in `*`/`_` or unwrap it (`toggle_marker`). Bold is Ubuntu Bold (`assets/fonts/`, Ubuntu Font
     Licence like egui's default Ubuntu Light, which has no bold), registered as the `bold` font family
     (`install_fonts`); italic is egui's slant.
   - Crop: not a tool but a mode for one image. Enter it with the Crop button or C (shown/working only
     while exactly one image is selected) or by double-clicking an image. The whole source image shows dimmed and
     outlined, the kept part bright with 8 handles (white bars with a dark rim, unlike the resize squares:
     L-shaped brackets at the corners, a bar along each edge; `crop_handle_bars`) that drag its edges/corners freely within the
     source; dragging inside it slides the kept part over the source. Enter, Escape, C, the Crop
     button, clicking elsewhere or switching tools finishes. Non-destructive: images store `uv`
     (the kept part of the texture, 0..1), so recropping can widen it again. After cropping,
     corner resizing keeps the cropped part's undistorted proportions. One undo step per crop session.
5. **Pan & zoom** — middle-button drag pans, and so does left-drag while holding Space (not while
   typing; open-hand cursor while held, closed hand while panning). A blocking widget over the canvas keeps that drag from objects
   and tools. Mouse wheel zooms around the cursor (one step per notch; on macOS, which
   reports slow notches as a fraction of a line, each wheel event is one step); Ctrl +/- zoom
   around the canvas center; Ctrl+0 resets. Steps of 20% (zoom = 100% + 20% × level, 20%–500%).
   Resizing the window scales the view along, around the canvas center: zoom is also multiplied by
   the canvas panel's size relative to `fit_size` (the smaller of the width/height ratios), which is
   cleared by Ctrl+0/clicking the percentage and set by fitting the window to a pasted image. The
   layers panel doesn't scale it (`fit_size` changes with the panel). Zooming from such an off-grid
   zoom first lands on the next 20% level in that direction (and drops the resize factor), so 100%
   stays reachable.
   Current zoom shows as a percentage at the right end of the toolbar; clicking it resets to 100%.
6. **Undo/redo** — Ctrl+Z undoes, Ctrl+Y or Ctrl+Shift+Z redoes (up to 200 steps). Covers every
   change to objects: paste, draw, text, move, resize, arrow points, delete. Not in history:
   selection, tool, pan/zoom.
7. **Canvas background** — swatch in the toolbar (next to the zoom level): transparent
   (shown as a checkerboard), white (default) or black. Copy uses it as the PNG background; with
   transparent, the PNG has real alpha (the offscreen render clears to transparent).
   Each background has a default ink: black on white/transparent, white on black. Switching the
   background while the drawing (or text) color is the old ink switches it to the new one (other colors
   are kept). When the ink changes, objects' black and white (colors and fills) swap both ways
   (`set_background`), so switching back restores them; objects on an image (above one, `mostly_inside`) are
   skipped. The background isn't in undo history (undo reverts the recoloring only).
8. **Style panel** — a compact bar just above the toolbar while Pencil, Arrow, Shape or Text is active,
   or while a stroke, arrow, line, shape or text is selected. It shows the current color and size; clicking
   either expands its options upwards, centered above that button (picking one collapses it again). The panels stack: toolbar, then the style panel (centered above the current tool's button, or with Select above the tool that draws the first selected object, `tool_anchor`), then the open options (`style_anchor` is the open button's center, set by `style_toggle`). Colors: black, white,
   red, orange, green, blue. Pen style (pencil and strokes): plain or hand-drawn. Arrow or line (the Arrow tool and arrows/lines). Arrow heads (arrows, not lines, so not in line mode; `ArrowHeads`/`Head` in `object.rs`): a start button (pointing left) and an end button (pointing right), each previewing that end and opening its options: none, open lines or solid triangle (default: none at the start, solid at the end). Size: line width (2/3/6/10px, for pencil and arrows; its button shows the stroke-width icon, the px in its tooltip) or font size
   (S/M/L/XL = 16/28/44/64px; non-preset sizes from resizing show as px); text and the Text tool
   also get a horizontal alignment button (left/center/right, word-processor style line icons) and, unless the targets are only inline text, a vertical one (top/middle/bottom, lines in a box); each sets only its own axis; shapes and the Shape tool also get "no border" among the line widths; text, shapes and the Text and Shape tools get a fill button
   (transparent or a palette color). Shapes also get a shape button (rectangle, ellipse, diamond), first in the bar.
   Text, arrows/lines and the Text and Arrow tools also get an outline button (`Outline` in `object.rs`: `on`, `color`,
   `width`; a capsule icon filled with its color): no outline, automatic color (`outline_color` in style.rs: black around
   light colors, white around the rest; a half black/white swatch), a palette color, and `OUTLINE_WIDTHS` (2/4/6px, how far
   it reaches out); picking a color or width turns it on. Arrows paint it as a wider pass under themselves (`paint_arrow`'s
   `outline`), text as copies of an outline-colored galley on two rings around it (`paint_text_outline`), also while editing.
   The Blur tool and selected blurs get only the three strength buttons, in the bar itself (no section).
   With a drawing tool it sets the style of new objects and also restyles the selected objects that tool draws
   (so the one just drawn; `drawing_tool`); with a selection it restyles every
   selected object that has that setting (undoable) and also becomes the default. Text has its own
   color (`text_color`), separate from the one strokes, arrows, lines and shapes share; recoloring a
   selection sets the default of each kind in it. A mixed
   selection of lines and text shows both a line width and a font size button; images are skipped. Arrow heads grow with the line width; an open head is one path so its point stays sharp when thick, and the line stops inside a solid head so its square end doesn't show.

9. **Menu bar** — top of the window (`menu_bar`).
   - File: Open image… (Ctrl+O; png/jpg/bmp/gif/webp via the `image` crate, placed like a paste at the
     canvas center, fitting the window if the canvas is empty), Save as… (Ctrl+S), Preferences… (Ctrl+,; see 12),
     Exit (Ctrl+W; works even while typing).
   - Edit: Undo, Redo, Cut, Copy, Paste, Delete, Select all, Duplicate, with shortcuts shown. Cut/Copy/Paste
     send `ViewportCommand::Request*`, so they produce the same clipboard events as the keys (and act on
     text being edited). A menu Paste goes to the canvas center, not under the pointer.
   - Cut (Ctrl+X) copies only the selected objects as a PNG and deletes them; Copy still copies
     everything. Exports carry their own snapshot of objects (`Export::new`) for that reason.
   - Help: View on GitHub (opens `repository` from Cargo.toml in the browser); Changelog (`changelog_window`:
     CHANGELOG.md built in with `include_str!`, a bold heading per `## ` release, "· this version" on the running
     one, its items as bullets, scrollable). After an update it opens on launch as "What's new" (`whats_new_since`):
     only the releases newer than `last_version` (in `Settings`, the version that ran last; none on a first run), and a
     Full changelog button; Check for updates… (native; see 11); About Deeraw — logo, name, tagline (`description` in Cargo.toml), version and author
     (`authors`), read with `env!`. Escape or × closes it. It also links the license.
10. **Remembered settings** — the style (color, sizes, pen, arrow heads, alignment, fill), the canvas
    background and the last file folder survive a relaunch, via eframe's `persistence` feature
    (`Settings` and `App::save` in `app/mod.rs`; `app.ron` in `%APPDATA%\deeraw\data`, localStorage on
    the web), as do the window size and position. Saved on exit and every 30 seconds. `#[serde(default)]`
    keeps older settings files loading as fields are added; unreadable ones are ignored. The drawing
    isn't kept, and the app always starts with Select.
11. **Update check** (native only, `app/update.rs`) — on launch a thread runs the system's `curl` (Windows 10
    1803+ and macOS have it; `CREATE_NO_WINDOW` on Windows) for `HEAD <repository>/releases/latest`, and reads the
    version from the redirect's `Location: …/releases/tag/vX.Y.Z` (no HTTP or JSON crate). If it's newer (compared
    by number), an "Update available" window links the release page (What's new) and offers, since Deeraw is
    distributed through the Microsoft Store and Homebrew rather than as release downloads, Open Microsoft Store
    (Windows; its apps.microsoft.com page) or `brew upgrade --cask deeraw` with Copy command (macOS), then Remind me later (no window
    on launch for 24h, `update_reminder`) and Skip this version (`skipped_version`); both are in `Settings`. Help >
    Check for updates… always shows the result, also "up to date" or a failure, and ignores both. Microsoft Store
    installs (running from an MSIX package, `GetCurrentPackageFullName`) don't check on launch. PRIVACY.md's
    "Update check" section describes this request.
12. **Preferences** (native only, `app/preferences.rs`; File > Preferences…, Ctrl+,) — `Preferences` in `Settings`:
    "When Deeraw opens": Take a screenshot (macOS/Windows; `screenshot_on_launch`, starts one like `--screenshot`
    but adds it to the empty canvas instead of starting a new drawing) and Check for updates (`check_for_updates`,
    on by default; Help > Check for updates… still works); "Text": Format markdown (`markdown`, on by default; off lays
    text out as typed via `markdown::set_enabled`, a global the app sets every frame, and Ctrl+B/I do nothing). Reset to defaults applies `Settings::default()`
    (`apply_settings`): style, background, file folder, skipped/reminded updates and these; not the window size.

## Current state

- Code layout (`src/`): `main.rs` (native window / web canvas setup, `mod`s), `object.rs` (`Object`, `Kind`, `TextFrame`),
  `style.rs` (`Style`, palette/sizes, `Background`), `history.rs` (undo), `geometry.rs` (resize/crop/zoom math),
  `stroke.rs` (pencil points, pressure, hand-drawn painting), `paint.rs` (painting objects), `markdown.rs` (text formatting, bold font), `mosaic.rs` (pixelation), `export.rs`
  (offscreen render, PNG), `widgets.rs` (style panel/menu buttons), `cursor.rs`. `app/mod.rs` holds `DeerawApp`,
  its state types and object operations; its `impl` continues in `app/canvas.rs` (input, tools, painting,
  text editing), `app/selection.rs` (handles, crop), `app/snap.rs` (arrow snapping/attaching), `app/blur.rs` (Blur tool), `app/panels.rs` (layers, toolbar, style panel),
  `app/menu.rs` (menu bar, About, Changelog), `app/update.rs` (update check), `app/preferences.rs` (Preferences window), `app/intro.rs` (empty-canvas intro), `app/files.rs` (paste/open, fit window, save, export) and `app/screenshot.rs` (screen capture, macOS and Windows).
  Constants live in the module that uses them. Methods called across `app/` files are `pub(super)`.
  Tests sit in each module's `mod tests`. Dependencies: eframe 0.37, image (decodes opened files), rfd (file/message dialogs), on macOS dispatch2/objc2/objc2-app-kit/objc2-foundation/block2 (screenshot window handling, the launcher's notification; already in winit's tree), on Windows arboard/windows-sys (screenshot clipboard and window handling; already in eframe's tree), png (already
  in eframe's tree; writes saves), chrono (local time for file names). Edition 2024. Dev builds optimize
  dependencies (`[profile.dev.package."*"]`), since unoptimized egui/wgpu made window dragging laggy. `cargo test` covers the
  geometry (resize, crop, zoom), stroke pressure and spline, undo/redo, selection, arrow snapping and PNG writing.
- App icon: `assets/deer.svg` is the source (dark variant: #242424 rounded square, white deer drawn as
  round-capped strokes). `python3 assets/make_icons.py` (ImageMagick + Pillow) renders it
  to `assets/icon.ico` (16–256px), embedded in the exe by `build.rs` (winresource, needs the Windows
  SDK's rc.exe), `assets/icon.png` (256px), the window/taskbar icon (`with_icon`), and the MSIX logos in `msix/Assets`.
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
- Dialogs (About, Changelog, Preferences, update) go through `dialog` in widgets.rs, so they share margins, spacing and
  button size (the title bar keeps egui's default margin; `Window` pads it with the frame's margin otherwise). Menus
  get roomier spacing from `menu_style` in menu.rs.
- An `Area` with a pivot other than LEFT_TOP is placed using last frame's size, so it's misplaced for a frame when its content resizes; `redo_frame_if_resized` (panels.rs) discards that frame (`ctx.request_discard`).
- Images are painted with linear sampling, so their screen rect is moved onto whole physical pixels
  (`snap_to_pixels` in paint.rs; blurs and the crop view follow); otherwise they look soft at 100%.
- Clipboard needs no extra crates: egui-winit emits `Event::PasteImage` (when the clipboard has
  no text) and `ctx.copy_image()` writes PNG via arboard.
- Not every Unicode glyph exists in egui's default fonts (e.g. `⬉` renders as a box); check
  toolbar icons visually.
- Pencil/Arrow use a custom bitmap crosshair and Text a custom I-beam (`outlined_cursor`) (`ctx.set_cursor_image`, which is sticky, so it's
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
- CI: `.github/workflows/build.yml` builds Windows (x64 zip) and macOS (universal `Deeraw.app` zip, the launcher inside) on every push/PR; a `v*` tag publishes a release.
- Microsoft Store: `msix/build.ps1` packages `target\release\deeraw.exe` as `deeraw-windows-x64.msix` (full-trust
  desktop app; `msix/AppxManifest.xml` with `$...$` placeholders, the logos in `msix/Assets` named for makepri's
  scale/targetsize qualifiers, one `resources.pri`). Version is Cargo.toml's plus `.0` (the Store wants the 4th
  part 0). The identity (`Briedis.Deeraw`, publisher `CN=5063D15F-...`, display name `Briedis`) is the app's in
  Partner Center (Product identity), as `build.ps1`'s parameter defaults. Store ID `9PNMW6XTFLBM`,
  https://apps.microsoft.com/detail/9PNMW6XTFLBM. Unsigned, since the Store signs it; CI attaches it
  to the GitHub release, to download and upload as a new Partner Center submission. To try it locally (Developer Mode):
  `Add-AppxPackage -Register $env:TEMP\deeraw-msix\AppxManifest.xml` after building, `Remove-AppxPackage` to undo.
  Packaged, `%APPDATA%` writes (settings, crash reports) go to `%LOCALAPPDATA%\Packages\<family>\LocalCache\Roaming`.
- Homebrew: `brew install --cask briedis/tap/deeraw`, from the tap repo https://github.com/briedis/homebrew-tap
  (`Casks/deeraw.rb`, installs `Deeraw.app` from the release's macOS zip and strips its quarantine flag, since
  it isn't notarized; homebrew/cask itself requires notarized apps). The release job bumps `version` and `sha256`
  there itself (needs the `HOMEBREW_TAP_TOKEN` secret, a fine-grained token with Contents read/write on the tap;
  skipped without it). Release steps: bump `version` in Cargo.toml, rename `## Unreleased` in CHANGELOG.md to
  `## X.Y.Z (date)`, commit, `git tag -a vX.Y.Z -m ...` (tags must be annotated here), push the tag. The release's
  notes are that changelog section.
- CHANGELOG.md: user-facing changes per release, newest first. Add a line under `## Unreleased` (create it if
  missing) with each commit that changes what users see or get; internal changes (refactors, docs, CI) stay out.
- macOS signing: CI signs Deeraw.app and the launcher with a self-signed "Deeraw Signing" code signing certificate
  (secrets `MACOS_CERT_P12`, base64 of the .p12, and `MACOS_CERT_PASSWORD`; imported into a temporary keychain,
  used by SHA-1 as `SIGN_ID`, which `macos-launcher/build.sh` signs with). The same certificate on every build lets
  macOS keep the Screen Recording permission across updates (ad-hoc signatures are identified by their hash,
  which changes each build). Without the secrets it signs ad-hoc. Losing the certificate means users grant
  the permission again once.
- macOS menu bar launcher (`macos-launcher/`): a separate Swift app (`main.swift`, `LSUIElement`, no Dock icon) that
  lives inside Deeraw.app (`Contents/Library/LoginItems/Deeraw Launcher.app`, so Spotlight doesn't list it) and
  shows the deer in the menu bar; left click takes a screenshot in the Deeraw.app it's in, right click shows
  Take Screenshot/Open Deeraw (no screenshot)/Quit. For a screenshot it opens Deeraw with `--screenshot` and also posts the
  distributed notification `com.briedis.deeraw.screenshot`, since a running Deeraw gets no arguments from macOS
  (`listen_for_launcher` in `app/screenshot.rs`, observed via objc2-foundation/block2). Either makes Deeraw start a
  screenshot once its window is key (`screenshot_until`, retried for up to `SCREENSHOT_START_FRAMES`). Once taken,
  a launcher screenshot replaces the drawing (`screenshot_new_drawing`, `new_drawing`; undo brings the old one back,
  and the capture fits the window like a paste onto an empty canvas); Escape cancels and keeps it. The toolbar's
  Screenshot button still adds to the drawing. Deeraw still accepts `--paste` (the launcher no longer passes it): a
  freshly started Deeraw pastes the clipboard on its first frame like Edit > Paste (`paste_on_start`; text on the
  clipboard is ignored then, `start_paste_frame`, so only an image is placed). It
  registers itself as a login item (`SMAppService`), and Deeraw starts it in the background on every launch
  (`start_launcher` in main.rs, `open -g`; nothing happens if it's running or Deeraw isn't run from its bundle). `build.sh [Deeraw.app]` builds it universal into that app
  (default `/Applications/Deeraw.app`) and re-signs the app; the menu bar PNGs are committed, rendered from
  `menubar-deer.svg` (the deer from `assets/deer.svg`, black, thicker strokes, no background).
- Git repo on `master`; commit after each working step.
- Never add a `Co-Authored-By:` trailer (or any other co-author credit) to a commit. Commits are
  authored by the repo owner alone, whoever or whatever wrote the code.
- Crash reports: a panic hook (`install_crash_report`, main.rs, native only) writes
  `crash-<time>.txt` (version, OS, panic message and place, backtrace) to the settings folder
  (`eframe::storage_dir("Deeraw")`, e.g. `%APPDATA%\Deeraw\data`) and shows a dialog with its
  path and the issues URL. The Windows zip ships `deeraw.pdb` so release backtraces have names.

## Planned next steps

Ideas, not commitments (eframe/egui was chosen for quick setup, easy mouse input and 2D painting):

1. Rectangle and ellipse shapes.
2. Save/load a drawing to a file (currently only PNG export).
3. Hand-drawn ("rough") style for shapes, arrows and lines (pencil strokes already have one).
4. SVG export.

Order and scope are open to change; confirm with the user before starting each step.

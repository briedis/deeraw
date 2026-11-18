# Features: toolbar and tools

"Ctrl" means Cmd on macOS (egui's `Modifiers::COMMAND`).

4. **Toolbar** — floating at the bottom of the canvas: icon buttons (tooltips name them and their key): first (not on the web, then a separator) Screenshot (macOS and Windows), Paste (a clipboard; like Edit > Paste, at the canvas center) and Copy (a page with a cut corner over another; the whole drawing, like Edit > Copy, faded while empty), then Select, Pencil, Arrow (it shows the line icon in line mode), Shape, Blur (a droplet), Text, plus Crop while exactly one image is selected; after a vertical separator, Undo and Redo (U-turn arrows,
   faded while there's nothing to undo/redo; the Edit menu items are disabled then too); after another, the canvas background (a single swatch that opens the others, like a style panel section) and the zoom level as a plain percentage (fixed width, so the bar keeps its size). The icons are traced from the Deeraw icon set (32×32 grid) and painted as strokes in `widgets.rs` (`Icon`, `icon_button`), so no SVG renderer is needed. The toolbar and the style panel follow the canvas background (`canvas_ui_visuals` in `style.rs`): white with slate icons on a white or transparent canvas, dark gray with light icons on black; the active tool is a blue button with a white icon.
   - Screenshot (macOS; `app/screenshot.rs`): orders the window out and runs `screencapture -i` on a thread (drag an
     area, Space for a window, Escape cancels); the capture is placed like an opened image. The window is hidden and
     shown through AppKit (`orderOut`, then `makeKeyAndOrderFront` on the main queue via dispatch2), not
     `ViewportCommand::Visible`: eframe runs no frames for a hidden window, so it could never show itself again; and
     hiding the app instead (Cmd+H style) loses activation, which macOS won't give back, so it returned behind other apps.
   - Screenshot (Windows; same file): minimizes the window and opens Snipping Tool's area selection
     (`ms-screenclip:`, as Win+Shift+S). Its only result is the clipboard, so a thread polls
     `GetClipboardSequenceNumber` and reads the image with arboard. The selection takes the foreground while
     open, so once another process's window has it again with no new image (after `CLIPBOARD_WAIT`), it was cancelled
     (by process, not window: it has an overlay per display, and the one dragged on takes the foreground);
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
   - Arrow: drag from start to end; stays in Arrow mode (a click on empty canvas, without a drag, deselects and goes back to Select, as with Shape and Blur). Arrows are point-to-point:
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
     Resizes freely; arrows attach to shapes like to text and images. Stays in Shape mode; a click on empty canvas goes back to Select.
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

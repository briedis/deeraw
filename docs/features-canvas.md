# Features: canvas, selection, view

"Ctrl" means Cmd on macOS (egui's `Modifiers::COMMAND`).

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
   **Arranging** (`app/arrange.rs`): Bring to front (Ctrl+Shift+]), Bring forward (Ctrl+]), Send backward (Ctrl+[),
   Send to back (Ctrl+Shift+[), in the Edit menu and the context menu. Right-clicking an object (with Select) selects it
   unless it's already selected and opens that menu (the four, Duplicate, Delete) at the pointer; it's a `Popup` on the
   canvas background, since arrows and shapes only have a widget near the pointer. What's on a selected image (`riders`)
   moves along, and blurs go with their image and never move alone, so they stay above it. One step goes past the
   nearest object the selection overlaps (non-overlapping ones would show no change), never past another selected one;
   items that would change nothing are disabled.
   **Multi-select:** Ctrl+click (canvas or layers panel) toggles an object in the selection;
   dragging on empty canvas draws a selection box that selects objects entirely inside it
   (Ctrl adds to the selection). Dragging any selected object moves them all. With several
   selected, each gets an outline and there are no resize handles. Hit-testing uses the bounding box, except
   arrows (see below).
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

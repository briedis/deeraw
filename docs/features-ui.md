# Features: style panel, menus, settings

"Ctrl" means Cmd on macOS (egui's `Modifiers::COMMAND`).

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
   - File: New drawing (Ctrl+N; `new_drawing`: saves the current one to History and empties the canvas, undo brings
     it back, so no confirmation; disabled while empty), Open image… (Ctrl+O; png/jpg/bmp/gif/webp via the `image` crate, placed like a paste at the
     canvas center, fitting the window if the canvas is empty), History… (native; see 13), Save as… (Ctrl+S), Preferences… (Ctrl+,; see 12),
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
    isn't kept (only as an image in History, see 13), and the app always starts with Select.

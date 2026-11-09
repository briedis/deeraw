# Architecture and technical notes

"Ctrl" means Cmd on macOS (egui's `Modifiers::COMMAND`).

- Code layout (`src/`): `main.rs` (native window / web canvas setup, `mod`s), `object.rs` (`Object`, `Kind`, `TextFrame`),
  `style.rs` (`Style`, palette/sizes, `Background`), `history.rs` (undo), `geometry.rs` (resize/crop/zoom math),
  `stroke.rs` (pencil points, pressure, hand-drawn painting), `paint.rs` (painting objects), `markdown.rs` (text formatting, bold font), `mosaic.rs` (pixelation), `export.rs`
  (offscreen render, PNG), `widgets.rs` (style panel/menu buttons), `cursor.rs`. `app/mod.rs` holds `DeerawApp`,
  its state types and object operations; its `impl` continues in `app/canvas.rs` (input, tools, painting,
  text editing), `app/selection.rs` (handles, crop), `app/snap.rs` (arrow snapping/attaching), `app/blur.rs` (Blur tool), `app/panels.rs` (layers, toolbar, style panel),
  `app/menu.rs` (menu bar, About, Changelog), `app/update.rs` (update check), `app/preferences.rs` (Preferences window), `app/intro.rs` (empty-canvas intro), `app/files.rs` (paste/open, fit window, save, export) and `app/screenshot.rs` (screen capture, macOS and Windows), `app/backup.rs` (when to save to History), `app/gallery.rs` (the History window);
  `history_store.rs` (History's SQLite database and its thread), `drawing.rs` (a drawing as History saves it, JSON).
  Constants live in the module that uses them. Methods called across `app/` files are `pub(super)`.
  Tests sit in each module's `mod tests`. Dependencies: eframe 0.37, image (decodes opened files), rfd (file/message dialogs), on macOS dispatch2/objc2/objc2-app-kit/objc2-foundation/block2 (screenshot window handling, the launcher's notification; already in winit's tree), on Windows arboard/windows-sys (screenshot clipboard and window handling; already in eframe's tree), rusqlite (native; History, SQLite bundled), serde_json (native; History's drawings), png (already
  in eframe's tree; writes saves), chrono (local time for file names). Edition 2024. Dev builds optimize
  dependencies (`[profile.dev.package."*"]`), because unoptimized egui/wgpu made window dragging laggy. `cargo test` covers the
  geometry (resize, crop, zoom), stroke pressure and spline, undo/redo, selection, arrow snapping, PNG writing and History (saving, reopening the database, when drawings are saved).
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
- Dialogs (About, Changelog, Preferences, update) go through `dialog` in widgets.rs (fixed, centered; History uses
  `movable_dialog`, the same look but movable and resizable), so they share margins, spacing and
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
- wgpu is set up in `wgpu_options` (`main.rs`). The surface uses `SurfaceConfig::LOW_LATENCY` (1 queued frame);
  `HIGH_THROUGHPUT` (2 frames) made window resizing jittery on Windows (the presented frame lagged the
  window size and was stretched to fit), and window dragging is smooth with 1 frame too.
  `InstanceFlags::VALIDATION_INDIRECT_CALL` is cleared because it stopped the app from starting on
  older GPUs: wgpu validates indirect draws with an internal compute pipeline built while the device
  is created, Metal's shader compiler fails to build it on an Intel Iris Pro ("Compiler encountered
  an internal error"), and `request_device` then fails with `Device(Lost)` before any window appears.
  Nothing here draws indirectly. Symptoms of it coming back: the app exits at startup with
  `Error: Wgpu(RequestDeviceError(... Device(Lost)))` and no window.
- Crash reports: a panic hook (`install_crash_report`, main.rs, native only) writes
  `crash-<time>.txt` (version, OS, panic message and place, backtrace) to the settings folder
  (`eframe::storage_dir("Deeraw")`, e.g. `%APPDATA%\Deeraw\data`) and shows a dialog with its
  path and the issues URL. The Windows zip ships `deeraw.pdb` so release backtraces have names.

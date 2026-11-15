# Features: updates, preferences, History

"Ctrl" means Cmd on macOS (egui's `Modifiers::COMMAND`).

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
    text out as typed via `markdown::set_enabled`, a global the app sets every frame, and Ctrl+B/I do nothing);
    "History": Save drawings to History (`history`, on by default; off, `backup_now` saves nothing, and File > History
    still opens older entries) and, under it (greyed out while off), Keep the last N drawings (`history_size`, default 20,
    1–`MAX_HISTORY_SIZE`; see 13). Reset to defaults applies `Settings::default()`
    (`apply_settings`): style, background, file folder, skipped/reminded updates and these; not the window size.
13. **History** (native only; `history_store.rs`, `drawing.rs`, `app/backup.rs`, `app/gallery.rs`) — drawings are kept in
    SQLite (rusqlite, `bundled`), `history.db` in the settings folder, WAL mode. An entry holds the drawing's objects as
    JSON (`drawing.rs`: `SavedDrawing` with a `version`, the background and `SavedObject`s; `Kind` derives serde with
    its Image variant skipped, and `SavedKind` stores an image as a key plus `uv`, falling back untagged to `Kind` for
    the rest), the flattened PNG (the Copy image, trimmed) and a 256px thumbnail. Images' pixels (`Pixels`, the source,
    unblurred) are stored once in the `image` table under `image_key` (size + a hash of the pixels, computed on the
    worker; only keys not stored yet are encoded), `entry_image` links entries to them, and unused ones are deleted with
    every save, delete and prune (`collect_garbage`). `PRAGMA user_version` is 1; `Store::init` migrates
    version 0 (flattened PNGs only) by adding `drawing`, `image`, `entry_image` in an immediate transaction, and
    version 0 entries open flattened.
    One entry per drawing, replaced by its later saves: a drawing starts at launch, at `new_drawing` (File > New drawing,
    the launcher screenshot) and when an entry is opened. Saved 3s (`AUTOSAVE_DELAY`) after it changes, once the canvas
    is settled (objects or background differ from `Backup::saved`), before `new_drawing` clears it, and on close (the
    close is cancelled until the render is handed over; `on_exit` joins the worker so the write finishes). Empty drawings
    aren't saved. Saves render through `export_step` (`ExportTarget::History`, queued in `Backup::exports`, so they don't
    clash with Copy) and hand the image and the export's objects to a thread (`Worker`), which serializes, encodes and
    writes in one transaction; it maps the app's drawing numbers to entry ids and sends back the list of entries
    (`Reply::Entries`) after each save. When it opens the database (first frame) it deletes all but the newest
    `history_size` entries (`Store::prune`, by last save); the file doesn't shrink, later saves reuse the space.
    File > History… opens a gallery window (`app/gallery.rs`, `show_history`; movable and resizable, starting centered
    with 4 columns; tiles stretch to fill its width, adding a column once they'd pass 256px thumbnails): tiles newest
    saved first, each the thumbnail (fitted into a 4:3 box, never scaled up, on a checkerboard for transparency) with its time ("Today 14:32", "Yesterday", "Oct 3, 18:20", the
    year if not this one; `time_label`) and size; it scrolls (`show_rows`), and only the visible rows' thumbnails are
    asked for (`Worker::thumbnail`, decoded on the worker, kept as textures in `Backup::thumbnails` with the save
    they're of, asked again once the entry is saved again). Clicking a tile loads it on the worker (`load_entry`: the
    JSON and its images decoded; a drawing that can't be read or is from a newer version opens flattened), closes the
    window and starts a new drawing (undo brings the old one back): `restore_drawing` sets the background
    (`set_background` on the empty canvas, so the drawing color follows), restores the objects with fresh ids
    (`drawing::restore` remaps arrows' bindings and blurs' images; an image whose pixels are missing is left out with
    its blurs), makes textures and mosaics (`image_kind` in files.rs), moves them to the canvas origin and fits the
    window to them like a pasted image (centered if it can't be resized). Entries without a drawing are placed like an
    opened image. The opened entry stays as it was: changes go to a new entry. Deleting: a hovered tile shows a trash
    button (`Icon::Trash`, `paint_icon`) in its corner, and right-click offers Open and Delete; either hides the tile at
    once, with "Deleted the drawing from …" and an Undo button above the grid (`Backup::pending_delete`). It's deleted
    for good (`Worker::delete`, which also forgets its drawing's entry, so that drawing's next save adds a new one)
    when another tile is deleted, the window closes or the app exits (`backup_exit`).


# Changelog

What changed in each release. Ctrl means Cmd on macOS.

## Unreleased

- Right-click an object for a menu to bring it to front, forward, backward or to back, duplicate or delete it.
  Also in the Edit menu, and with Ctrl+] / Ctrl+[ (one step) and Ctrl+Shift+] / Ctrl+Shift+[ (all the way).
- File > History…: your last 20 drawings as thumbnails, saved automatically as you work and when Deeraw closes,
  to open again. Opening one starts a new drawing with it (undo brings back the one you had). File > Preferences turns it off,
  or sets how many drawings it keeps (20 by default); older ones are deleted when Deeraw opens.
- Delete drawings from History: hover one for its delete button, or right-click it.
- File > New drawing (Ctrl+N) starts over; the current drawing is kept in History, and undo brings it back.

## 0.9.0 (2026-10-27)

- Roomier menus, and the Changelog, About, Preferences and update windows share one roomier layout.
- After an update, Deeraw opens with what's new: the changes in every release since the version you had.
- Text, arrows and lines can have an outline (style panel), so they stand out on busy screenshots: automatic
  (black or white, whichever contrasts) or a palette color, 2, 4 or 6px.
- Preferences: turn off markdown formatting in text, so it shows exactly as typed.
- Text formatting uses Slack's markers: `*bold*`, `_italic_` and `~strikethrough~` (Ctrl+B and Ctrl+I add them).

## 0.8.1 (2026-10-27)

- The mouse wheel no longer zooms the canvas while a menu or a window (Preferences, Changelog, About) is open.

## 0.8.0 (2026-10-27)

- Line is now a mode of the Arrow tool (one toolbar button fewer): pick Arrow or Line in the style panel, or press
  L. Picking it with arrows or lines selected turns them into the other.
- Text alignment has separate horizontal and vertical buttons; the vertical one shows only for text boxes.
- Switching between a light and a dark background swaps black and white in the drawing, so it doesn't vanish
  (other colors, and things on an image, stay as they are).
- Changing the style right after drawing something (while its tool is still active) restyles what you just drew.

## 0.7.0 (2026-10-26)

- Moving an image takes along the text, drawings, shapes and images on it (hold Alt to move just the image).
- File > Preferences… (Ctrl+,): take a screenshot when Deeraw opens, turn off the update check, and reset all
  remembered settings to their defaults.
- Help > Changelog shows what changed in each release.
- Deeraw tells you when a new version is out (not for Microsoft Store installs, which the Store updates), with a
  link to the Store on Windows and the Homebrew command on macOS, Remind me later and Skip this version; Help > Check for updates… checks any time.
- The privacy policy now covers Microsoft Store installs and allows a check for new versions on GitHub.

## 0.6.2 (2026-10-24)

- Windows: a Microsoft Store package (MSIX) is built with every release.

## 0.6.1 (2026-10-23)

- macOS: the app is signed with the same certificate on every release, so the Screen Recording permission
  survives updates.
- The Homebrew cask is updated automatically on release.

## 0.6.0 (2026-10-23)

- An intro on the empty canvas at startup: paste, take a screenshot or open an image, or just start drawing.
- Arrows attached at one end move along with their object instead of stretching.
- The Arrow and Line tools show every object's attachment circles, also before drawing.
- Crop handles are white corner brackets and edge bars, distinct from the resize handles.
- Blurs: dragging over one with Select moves the image under it; click a blur to select it. Clicking empty canvas
  with the Blur tool goes back to Select, and a blur picked with Select goes back to Select after clicking
  anything else.
- Toolbar and style panel buttons react to any press and release over them.
- Added a privacy policy.

## 0.5.0 (2026-10-21)

- Ctrl+W closes the window.
- macOS: a click on the menu bar launcher always takes a screenshot into a new drawing, also while Deeraw is
  running (Cmd+Z brings the old drawing back; Escape cancels and keeps it).

## 0.4.0 (2026-10-21)

- Windows: Screenshot button, using Snipping Tool's area selection.
- macOS: the menu bar launcher lives inside Deeraw.app, so Spotlight doesn't list it, and Deeraw starts it.

## 0.3.0 (2026-10-20)

- macOS: Screenshot button, and a menu bar launcher that takes a screenshot into Deeraw.
- Basic markdown in text: bold, italic, strikethrough, code, headings and lists; Ctrl+B / Ctrl+I.
- Pasting text places it as a text object.
- Center alignment snapping with guides; text alignment and a background per line.
- Free arrow and line ends snap to horizontal, vertical and 45°.
- Smoother pencil strokes.
- Shapes can have no border; text has its own color.
- Undo and Redo buttons in the toolbar; Help > View on GitHub.
- Resizing the window scales the view.
- Images too big for the GPU are scaled down instead of crashing; crashes write a report.
- macOS: one zoom step per mouse wheel notch.

## 0.2.1 (2026-10-19)

First release.

- Paste or open images; Ctrl+C copies the drawing as a PNG trimmed to its content; Ctrl+S saves it.
- Tools: pencil (plain or hand-drawn), arrows and lines that attach to objects, rectangles, ellipses and diamonds,
  blur (pixelation) and text boxes.
- Select, multi-select, move, resize, duplicate, crop, undo/redo, pan and zoom.
- Style panel for colors, sizes and arrow heads; transparent, white or black canvas; the style is remembered.
- Layers panel (Ctrl+L), menu bar and keyboard shortcuts.

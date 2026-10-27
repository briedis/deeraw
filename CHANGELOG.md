# Changelog

What changed in each release. Ctrl means Cmd on macOS.

## Unreleased

- Line is now a mode of the Arrow tool (one toolbar button fewer): pick Arrow or Line in the style panel, or press
  L. Picking it with arrows or lines selected turns them into the other.
- Text alignment has separate horizontal and vertical buttons; the vertical one shows only for text boxes.
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

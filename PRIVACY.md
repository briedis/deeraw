# Privacy Policy

_Last updated: October 21, 2026_

Deeraw is a drawing app that runs entirely on your device. It does not collect, send or sell any
personal data. It has no accounts, analytics, telemetry, ads or tracking, and it makes no network
requests of its own.

## What stays on your device

- **Your drawings** exist only in the app while it's open. They are written to a file only when you
  save one (Ctrl+S) to a place you choose, and to the clipboard only when you copy (Ctrl+C or Ctrl+X).
- **Settings**: the last used style (colors, widths, font size), the canvas background, the folder
  you last saved to and the window size are remembered between launches. They are stored in Deeraw's
  data folder:
  - Windows: `%APPDATA%\Deeraw\data`
  - macOS: `~/Library/Application Support/Deeraw`
  - Linux: `~/.local/share/deeraw`
  - Web version: your browser's local storage for the site.
- **Crash reports**: if Deeraw crashes, it writes a report (app version, operating system, the error
  and where in the code it happened) to the data folder above and tells you where it is. It is
  never sent anywhere; you choose whether to attach it to a GitHub issue.

Deleting the data folder (or clearing the site's data in your browser) removes everything Deeraw
has stored.

## Clipboard and screen

- Deeraw reads the clipboard only when you paste (Ctrl+V) or start it with `--paste`.
- The **Screenshot** button captures only the area you select, and the capture stays on the canvas:
  - On macOS it uses the system's `screencapture`, through a temporary file that is deleted right away.
  - On Windows it uses Snipping Tool's area selection, which puts the capture on the clipboard, where
    Deeraw reads it from. It stays there (and in Windows' clipboard history, if that's on), and
    Snipping Tool may save it according to its own settings; see Microsoft's privacy statement.
- Blur pixelates part of an image on the canvas and in exports. The original image is kept while you
  edit (so a blur can be moved or removed), but exported PNGs contain only the pixelated version.

## Links

The menu's links (the GitHub repository and the license) open in your browser; what happens there is
covered by those sites' own policies.

## Web version

The web version runs in your browser in the same way: drawings aren't uploaded, and settings are kept
in the browser's local storage. Whoever hosts the page may keep ordinary web server logs (such as IP
addresses) for loading it.

## Children

Deeraw collects no data from anyone, including children.

## Changes

Changes to this policy are published in this file, with the date above updated.

## Contact

Questions: open an issue at https://github.com/briedis/deeraw/issues.

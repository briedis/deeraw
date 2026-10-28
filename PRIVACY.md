# Privacy Policy

_Last updated: October 24, 2026_

Deeraw is a drawing app that runs entirely on your device. It does not collect, send or sell any
personal data. It has no accounts, analytics, telemetry, ads or tracking. The only network request
it may make of its own is a check for a newer version (see below); your drawings never leave your
device unless you save, copy or share them yourself.

## What stays on your device

- **Your drawings** are written to a file when you save one (Ctrl+S) to a place you choose, and to the
  clipboard only when you copy (Ctrl+C or Ctrl+X).
- **History** (not in the web version): as you draw, each drawing is also kept as an image, with a small
  preview, in `history.db` in the data folder below, so File > History… can open it again. It keeps your
  last 20 drawings, or as many as set in File > Preferences; older ones are deleted when Deeraw opens.
  Turning off "Save drawings to History" there stops it from saving any more.
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

## Update check

Deeraw may check whether a newer version is available by asking GitHub for the latest Deeraw release
(the public page at https://github.com/briedis/deeraw/releases). This request contains nothing about
you or your drawings, and no identifier: only what any web request carries, such as your IP address
and the app's name and version. GitHub's privacy statement covers what GitHub does with it. The answer
is used only to tell you that an update exists; Deeraw doesn't download or install anything without
you choosing to.

## Microsoft Store

If you install Deeraw from the Microsoft Store, Microsoft handles the download and updates, and may
share aggregated, anonymous reports with the developer (such as install counts and crash reports from
Windows Error Reporting, depending on your Windows diagnostic settings). That is covered by Microsoft's
privacy statement; Deeraw itself sends nothing.

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

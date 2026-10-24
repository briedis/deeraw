<p align="center">
  <img src="assets/deer.svg" width="128" alt="Deeraw logo">
</p>

<h1 align="center">Deeraw</h1>

<p align="center">An opinionated drawing app inspired by Excalidraw, written in Rust.</p>

## Features

- **Paste & annotate**: paste or open an image, then draw on top of it. The window fits itself to the first image.
  Pasted text becomes a text object.
- **Screenshot** (macOS and Windows): drag over part of the screen and it lands on the canvas, ready to annotate.
  On Windows it uses Snipping Tool's area selection, so the capture is also left on the clipboard.
- **Tools**: pencil (plain or hand-drawn, speed-sensitive strokes), arrows and lines (with angle snapping),
  rectangles, ellipses and diamonds (outlined, filled or both), blur (pixelates part of an image, safe for hiding
  things in screenshots) and text.
- **Text**: single-line or wrapping text boxes with alignment and a background, and basic markdown:
  `**bold**`, `*italic*`, `~~strike~~`, `` `code` ``, headings and lists.
- **Arrows that connect**: arrow ends attach to images, text and shapes and follow them when they move.
- **Edit**: select, multi-select, move, resize, duplicate, non-destructive image crop, alignment guides that snap
  centers together, and undo/redo.
- **Style**: colors, line widths and font sizes; white, black or transparent canvas. The last style is remembered.
- **Pan & zoom** with the mouse wheel, middle button or Space + drag.
- **Export**: Ctrl+C copies the drawing as a PNG cropped to its content, Ctrl+X copies just the selection, and
  Ctrl+S saves to a file.

Ctrl means Cmd on macOS.

## Install

### macOS

With [Homebrew](https://brew.sh):

```sh
brew install --cask briedis/tap/deeraw
```

`brew upgrade` updates it later. This installs `Deeraw.app` into `/Applications`. Inside it is
**Deeraw Launcher** (`Contents/Library/LoginItems`, so it doesn't show up in Spotlight), a deer in the menu bar
that Deeraw starts and that starts at login. Click it to take a screenshot: once taken, it replaces
the drawing with a new one (Cmd+Z brings the old one back); press Escape to cancel and keep the
drawing. Right click for its menu (Take Screenshot, Open Deeraw without a screenshot, Quit Launcher). If you don't want it, quit it and remove it from System Settings → General → Login Items.

Without Homebrew: download `deeraw-macos-universal.zip` from [Releases](https://github.com/briedis/deeraw/releases),
unzip it and move `Deeraw.app` to `/Applications`. It isn't notarized by Apple, so macOS blocks it at first:
open it, then allow it under System Settings → Privacy & Security → **Open Anyway** (or run
`xattr -dr com.apple.quarantine /Applications/Deeraw.app`).

The Screenshot button needs the Screen Recording permission; macOS asks for it the first time.

### Windows

Download `deeraw-windows-x64.zip` from [Releases](https://github.com/briedis/deeraw/releases), unzip it anywhere and
run `deeraw.exe`. Windows SmartScreen may warn about an unknown publisher: choose **More info** → **Run anyway**.

## Build

```sh
cargo run --release
```

For the web build, install [Trunk](https://trunkrs.dev) and the `wasm32-unknown-unknown` target, then run
`trunk serve --release`.

## Releases

Every push builds Windows and macOS on GitHub Actions (`.github/workflows/build.yml`). To publish a release:

1. Bump `version` in `Cargo.toml` (the macOS app's version comes from it), and in [CHANGELOG.md](CHANGELOG.md)
   rename `## Unreleased` to `## 0.4.0 (date)`. Commit.
2. Tag it with an annotated tag and push both:
   ```sh
   git tag -a v0.4.0 -m "Version 0.4.0"
   git push && git push origin v0.4.0
   ```
   The tag's build publishes a GitHub release with the zips and the Store's MSIX, its notes taken from the
   changelog, and updates the Homebrew cask in [briedis/homebrew-tap](https://github.com/briedis/homebrew-tap).
3. Submit the release's `deeraw-windows-x64.msix` in Partner Center as a Store update.

See [CHANGELOG.md](CHANGELOG.md) for what changed in each release.

## License

[PolyForm Shield 1.0.0](LICENSE.md): free to use, including commercially, except to build a competing product.

## Privacy

Deeraw collects no data; at most it may check GitHub for a newer version. See [PRIVACY.md](PRIVACY.md).

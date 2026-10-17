<p align="center">
  <img src="assets/deer.svg" width="128" alt="Deeraw logo">
</p>

<h1 align="center">Deeraw</h1>

<p align="center">An opinionated drawing app inspired by Excalidraw, written in Rust.</p>

## Features

- **Paste & annotate**: paste or open an image, then draw on top of it. The window fits itself to the first image.
- **Tools**: pencil (plain or hand-drawn, speed-sensitive strokes), arrows, lines and text (single-line or wrapping text boxes with alignment and a background).
- **Edit**: select, multi-select, move, resize, duplicate, non-destructive image crop, and undo/redo.
- **Style**: colors, line widths and font sizes; white, black or transparent canvas.
- **Pan & zoom** with the mouse wheel and middle button.
- **Export**: Ctrl+C copies the drawing as a PNG cropped to its content, and Ctrl+S saves it to a file.

## Download

Windows and macOS builds are on the [Releases](https://github.com/briedis/deeraw/releases) page.
The macOS app is unsigned, so right-click it and choose **Open** the first time.

## Build

```sh
cargo run --release
```

## License

[PolyForm Shield 1.0.0](LICENSE.md): free to use, including commercially, except to build a competing product.

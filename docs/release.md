# Build, release and packaging

"Ctrl" means Cmd on macOS (egui's `Modifiers::COMMAND`).

- App icon: `assets/deer.svg` is the source (dark variant: #242424 rounded square, white deer drawn as
  round-capped strokes). `python3 assets/make_icons.py` (ImageMagick + Pillow) renders it
  to `assets/icon.ico` (16–256px), embedded in the exe by `build.rs` (winresource, needs the Windows
  SDK's rc.exe), `assets/icon.png` (256px), the window/taskbar icon (`with_icon`), and the MSIX logos in `msix/Assets`.
- License: PolyForm Shield 1.0.0 (`LICENSE.md`, with a `Required Notice:` copyright line that
  must stay; `license` in Cargo.toml). Any use is allowed, commercial included, except providing a
  product that competes with Deeraw. Source-available, not OSI open source.
- Release builds use `windows_subsystem = "windows"` (no console window); debug builds keep the console.
- CI: `.github/workflows/build.yml` builds Windows (x64 zip) and macOS (universal `Deeraw.app` zip, the launcher inside) on every push/PR; a `v*` tag publishes a release.
- Microsoft Store: `msix/build.ps1` packages `target\release\deeraw.exe` as `deeraw-windows-x64.msix` (full-trust
  desktop app; `msix/AppxManifest.xml` with `$...$` placeholders, the logos in `msix/Assets` named for makepri's
  scale/targetsize qualifiers, one `resources.pri`). Version is Cargo.toml's plus `.0` (the Store wants the 4th
  part 0). The identity (`Briedis.Deeraw`, publisher `CN=5063D15F-...`, display name `Briedis`) is the app's in
  Partner Center (Product identity), as `build.ps1`'s parameter defaults. Store ID `9PNMW6XTFLBM`,
  https://apps.microsoft.com/detail/9PNMW6XTFLBM. Unsigned, since the Store signs it; CI attaches it
  to the GitHub release, to download and upload as a new Partner Center submission. To try it locally (Developer Mode):
  `Add-AppxPackage -Register $env:TEMP\deeraw-msix\AppxManifest.xml` after building, `Remove-AppxPackage` to undo.
  Packaged, `%APPDATA%` writes (settings, crash reports) go to `%LOCALAPPDATA%\Packages\<family>\LocalCache\Roaming`.
- Homebrew: `brew install --cask briedis/tap/deeraw`, from the tap repo https://github.com/briedis/homebrew-tap
  (`Casks/deeraw.rb`, installs `Deeraw.app` from the release's macOS zip and strips its quarantine flag, since
  it isn't notarized; homebrew/cask itself requires notarized apps). The release job bumps `version` and `sha256`
  there itself (needs the `HOMEBREW_TAP_TOKEN` secret, a fine-grained token with Contents read/write on the tap;
  skipped without it). Release steps: bump `version` in Cargo.toml, rename `## Unreleased` in CHANGELOG.md to
  `## X.Y.Z (date)`, commit, `git tag -a vX.Y.Z -m ...` (tags must be annotated here), push the tag. The release's
  notes are that changelog section.
- macOS signing: CI signs Deeraw.app and the launcher with a self-signed "Deeraw Signing" code signing certificate
  (secrets `MACOS_CERT_P12`, base64 of the .p12, and `MACOS_CERT_PASSWORD`; imported into a temporary keychain,
  used by SHA-1 as `SIGN_ID`, which `macos-launcher/build.sh` signs with). The same certificate on every build lets
  macOS keep the Screen Recording permission across updates (ad-hoc signatures are identified by their hash,
  which changes each build). Without the secrets it signs ad-hoc. Losing the certificate means users grant
  the permission again once.
- macOS menu bar launcher (`macos-launcher/`): a separate Swift app (`main.swift`, `LSUIElement`, no Dock icon) that
  lives inside Deeraw.app (`Contents/Library/LoginItems/Deeraw Launcher.app`, so Spotlight doesn't list it) and
  shows the deer in the menu bar; left click takes a screenshot in the Deeraw.app it's in, right click shows
  Take Screenshot/Open Deeraw (no screenshot)/Quit. For a screenshot it opens Deeraw with `--screenshot` and also posts the
  distributed notification `com.briedis.deeraw.screenshot`, since a running Deeraw gets no arguments from macOS
  (`listen_for_launcher` in `app/screenshot.rs`, observed via objc2-foundation/block2). Either makes Deeraw start a
  screenshot once its window is key (`screenshot_until`, retried for up to `SCREENSHOT_START_FRAMES`). Once taken,
  a launcher screenshot replaces the drawing (`screenshot_new_drawing`, `new_drawing`; undo brings the old one back,
  and the capture fits the window like a paste onto an empty canvas); Escape cancels and keeps it. The toolbar's
  Screenshot button adds to the drawing. Deeraw accepts `--paste` (not passed by the launcher): a
  freshly started Deeraw pastes the clipboard on its first frame like Edit > Paste (`paste_on_start`; text on the
  clipboard is ignored then, `start_paste_frame`, so only an image is placed). It
  registers itself as a login item (`SMAppService`, macOS 13+; it's built for macOS 11 like Deeraw, so on 11–12 it
  only runs once Deeraw starts it), and Deeraw starts it in the background on every launch
  (`start_launcher` in main.rs, `open -g`; nothing happens if it's running or Deeraw isn't run from its bundle). `build.sh [Deeraw.app]` builds it universal into that app
  (default `/Applications/Deeraw.app`) and re-signs the app; the menu bar PNGs are committed, rendered from
  `menubar-deer.svg` (the deer from `assets/deer.svg`, black, thicker strokes, no background).

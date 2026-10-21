#!/bin/sh
# Builds "Deeraw Launcher.app" (a menu bar icon that opens the Deeraw.app it's inside) as a universal
# binary into Deeraw.app/Contents/Library/LoginItems, so it doesn't show up in Spotlight on its own, and
# re-signs Deeraw.app. It registers itself as a login item on first launch.
# Usage: build.sh [Deeraw.app]   (default /Applications/Deeraw.app; its icon and version are used for the launcher)
# The menu bar PNGs are committed; after editing menubar-deer.svg, regenerate them with
# rsvg-convert -w 18 -h 18 menubar-deer.svg -o menubar-deer.png (and -w 36 -h 36 for @2x).
set -e
DEERAW="$(cd "${1:-/Applications/Deeraw.app}" && pwd)"
APP="$DEERAW/Contents/Library/LoginItems/Deeraw Launcher.app"
VERSION=$(/usr/libexec/PlistBuddy -c "Print CFBundleShortVersionString" "$DEERAW/Contents/Info.plist")
cd "$(dirname "$0")"
TMP=$(mktemp -d)

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
swiftc -O -target arm64-apple-macos13 main.swift -o "$TMP/arm64"
swiftc -O -target x86_64-apple-macos13 main.swift -o "$TMP/x86_64"
lipo -create -output "$APP/Contents/MacOS/DeerawLauncher" "$TMP/arm64" "$TMP/x86_64"
cp menubar-deer.png menubar-deer@2x.png "$APP/Contents/Resources/"
cp "$DEERAW/Contents/Resources/deeraw.icns" "$APP/Contents/Resources/" 2>/dev/null || true

cat > "$APP/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Deeraw Launcher</string>
  <key>CFBundleIdentifier</key><string>com.briedis.deeraw.launcher</string>
  <key>CFBundleExecutable</key><string>DeerawLauncher</string>
  <key>CFBundleIconFile</key><string>deeraw</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>LSUIElement</key><true/>
</dict>
</plist>
EOF

codesign --force --deep --sign - "$APP"
# Deeraw.app's signature covers its contents, so adding the launcher invalidated it.
codesign --force --sign - "$DEERAW"
echo "Built $APP"

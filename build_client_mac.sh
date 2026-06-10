#!/bin/bash
set -e

cargo build -p rustopus-client --release

APP="target/release/Rustopus Client.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/rustopus-client "$APP/Contents/MacOS/rustopus-client"

# Build the .icns from the source PNG (64x64 — don't upscale past it)
ICONSET="$(mktemp -d)/octopus.iconset"
mkdir -p "$ICONSET"
SRC="client/src/assets/images/octopus.png"
sips -z 16 16 "$SRC" --out "$ICONSET/icon_16x16.png" >/dev/null
sips -z 32 32 "$SRC" --out "$ICONSET/icon_16x16@2x.png" >/dev/null
sips -z 32 32 "$SRC" --out "$ICONSET/icon_32x32.png" >/dev/null
cp "$SRC" "$ICONSET/icon_32x32@2x.png"
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/octopus.icns"
rm -rf "$(dirname "$ICONSET")"

cat > "$APP/Contents/Info.plist" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>Rustopus Client</string>
    <key>CFBundleDisplayName</key><string>Rustopus Client</string>
    <key>CFBundleIdentifier</key><string>hu.orinkhungary.rustopus-client</string>
    <key>CFBundleExecutable</key><string>rustopus-client</string>
    <key>CFBundleIconFile</key><string>octopus</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleVersion</key><string>0.1.0</string>
    <key>CFBundleShortVersionString</key><string>0.1.0</string>
    <key>LSMinimumSystemVersion</key><string>10.13</string>
    <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
EOF

echo "Built: $APP"

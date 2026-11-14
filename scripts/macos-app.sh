#!/bin/sh
# Arma Orfeon.app a partir del ejecutable de la app (uso: macos-app.sh BINARIO DESTINO.app VERSION)
set -eu
bin="$1"; app="$2"; version="${3:-0.0.0}"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$bin" "$app/Contents/MacOS/Orfeon"
chmod +x "$app/Contents/MacOS/Orfeon"
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cp "$script_dir/../assets/Orfeon.icns" "$app/Contents/Resources/"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Orfeón</string>
  <key>CFBundleDisplayName</key><string>Orfeón</string>
  <key>CFBundleIdentifier</key><string>ar.com.estuariolabs.orfeon</string>
  <key>CFBundleIconFile</key><string>Orfeon.icns</string>
  <key>CFBundleExecutable</key><string>Orfeon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSApplicationCategoryType</key><string>public.app-category.music</string>
</dict>
</plist>
PLIST

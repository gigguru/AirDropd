#!/usr/bin/env bash
# Build AirDropd.app for macOS (Apple Silicon or Intel).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "Building AirDropd (release)…"
cargo build --release --manifest-path apple/Cargo.toml

BIN="$ROOT/target/release/AirDropd"
if [[ ! -f "$BIN" ]]; then
  echo "error: $BIN was not produced" >&2
  exit 1
fi

APP="$ROOT/apple/dist/AirDropd.app"
MACOS="$APP/Contents/MacOS"
RES="$APP/Contents/Resources"

rm -rf "$APP"
mkdir -p "$MACOS" "$RES"

cp "$BIN" "$MACOS/AirDropd"
chmod +x "$MACOS/AirDropd"
cp apple/packaging/Info.plist "$APP/Contents/Info.plist"

PNG="$ROOT/assets/airdropd-icon.png"
if [[ -f "$PNG" ]]; then
  ICONSET="$(mktemp -d)/AppIcon.iconset"
  mkdir -p "$ICONSET"
  sips -z 16 16     "$PNG" --out "$ICONSET/icon_16x16.png"      >/dev/null
  sips -z 32 32     "$PNG" --out "$ICONSET/icon_16x16@2x.png"    >/dev/null
  sips -z 32 32     "$PNG" --out "$ICONSET/icon_32x32.png"      >/dev/null
  sips -z 64 64     "$PNG" --out "$ICONSET/icon_32x32@2x.png"    >/dev/null
  sips -z 128 128   "$PNG" --out "$ICONSET/icon_128x128.png"     >/dev/null
  sips -z 256 256   "$PNG" --out "$ICONSET/icon_128x128@2x.png" >/dev/null
  sips -z 256 256   "$PNG" --out "$ICONSET/icon_256x256.png"     >/dev/null
  sips -z 512 512   "$PNG" --out "$ICONSET/icon_256x256@2x.png"  >/dev/null
  sips -z 512 512   "$PNG" --out "$ICONSET/icon_512x512.png"     >/dev/null
  sips -z 1024 1024 "$PNG" --out "$ICONSET/icon_512x512@2x.png"  >/dev/null
  iconutil -c icns "$ICONSET" -o "$RES/AppIcon.icns"
fi

SIZE=$(du -sh "$APP" | cut -f1)
ARCH="$(uname -m)"
ZIP="$ROOT/dist/AirDropd-macos-${ARCH}.zip"
mkdir -p "$ROOT/dist"
rm -f "$ZIP"
ditto -c -k --sequesterRsrc --keepParent "$APP" "$ZIP"
ZIP_SIZE=$(du -h "$ZIP" | cut -f1)

echo ""
echo "Built $APP ($SIZE)"
echo "Download file: $ZIP ($ZIP_SIZE)"
echo "Open with:  open \"$APP\""
echo "Or copy to Applications:  cp -R \"$APP\" /Applications/"

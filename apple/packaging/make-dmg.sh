#!/usr/bin/env bash
# Professional macOS drag-to-Applications DMG with branded wallpaper.
set -euo pipefail

usage() {
    echo "Usage: make-dmg.sh <AirDropd.app> <output.dmg> [logo.png]" >&2
    exit 1
}

[[ $# -ge 2 ]] || usage

APP_SRC="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
OUT_DMG="$(cd "$(dirname "$2")" && pwd)/$(basename "$2")"
LOGO="${3:-$(cd "$(dirname "$0")/../.." && pwd)/assets/airdropd-icon.png}"

if [[ ! -d "$APP_SRC" ]]; then
    echo "App bundle not found: $APP_SRC" >&2
    exit 1
fi
if [[ ! -f "$LOGO" ]]; then
    echo "Logo not found: $LOGO" >&2
    exit 1
fi

PACKAGING="$(cd "$(dirname "$0")" && pwd)"
BG_DIR="$PACKAGING/dmg-background"
BG_1X="$BG_DIR/background.png"
BG_2X="$BG_DIR/background@2x.png"
VOL_NAME="AirDropd"
MOUNT="/Volumes/$VOL_NAME"

mkdir -p "$BG_DIR" "$(dirname "$OUT_DMG")"
chmod +x "$PACKAGING/generate_dmg_background.swift"
"$PACKAGING/generate_dmg_background.swift" "$LOGO" "$BG_1X" "$BG_2X"

WORK="$(mktemp -d)"
RW_DMG="$WORK/staging.sparseimage"
cleanup() {
    hdiutil detach "$MOUNT" >/dev/null 2>&1 || true
    rm -rf "$WORK"
}
trap cleanup EXIT

APP_KB="$(du -sk "$APP_SRC" | awk '{print $1}')"
DMG_MB=$(( (APP_KB / 1024) + 64 ))
[[ "$DMG_MB" -lt 200 ]] && DMG_MB=200

hdiutil detach "$MOUNT" >/dev/null 2>&1 || true
hdiutil create -size "${DMG_MB}m" -type SPARSE -volname "$VOL_NAME" -fs HFS+ -ov "$RW_DMG" >/dev/null
hdiutil attach -readwrite -noverify -noautoopen "$RW_DMG" >/dev/null

cp -R "$APP_SRC" "$MOUNT/"
ln -s /Applications "$MOUNT/Applications"
mkdir -p "$MOUNT/.background"
cp "$BG_1X" "$MOUNT/.background/background.png"
cp "$BG_2X" "$MOUNT/.background/background@2x.png"
sips -s format tiff "$BG_1X" --out "$MOUNT/.background/background.tiff" >/dev/null
sync
sleep 2

BG_TIFF="$MOUNT/.background/background.tiff"
if command -v SetFile >/dev/null 2>&1; then
    SetFile -a V "$MOUNT/.background" || true
fi

/usr/bin/osascript <<APPLESCRIPT
tell application "Finder"
    activate
    delay 1
    tell disk "$VOL_NAME"
        open
        delay 1
        set current view of container window to icon view
        set toolbar visible of container window to false
        set statusbar visible of container window to false
        set the bounds of container window to {200, 120, 860, 520}
        set viewOptions to the icon view options of container window
        set arrangement of viewOptions to not arranged
        set icon size of viewOptions to 128
        set background picture of viewOptions to (POSIX file "$BG_TIFF" as alias)
        set position of item "AirDropd.app" of container window to {180, 200}
        set position of item "Applications" of container window to {480, 200}
        close
        open
        update without registering applications
        delay 2
    end tell
end tell
APPLESCRIPT

chmod -Rf go-w "$MOUNT" || true
bless --folder "$MOUNT" --openfolder "$MOUNT" >/dev/null 2>&1 || true

hdiutil detach "$MOUNT" >/dev/null
rm -f "$OUT_DMG"
hdiutil convert "$RW_DMG" -format UDZO -imagekey zlib-level=9 -o "$OUT_DMG" >/dev/null

echo "Created $OUT_DMG"

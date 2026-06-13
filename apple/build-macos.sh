#!/usr/bin/env bash
# Build AirDropd.app for macOS (native arch, optional cross-target, optional universal).
# Outputs:
#   apple/dist/AirDropd.app
#   dist/AirDropd-macos-{arm64|x86_64|universal}.zip
#   dist/AirDropd Setup.dmg  (professional drag-to-Applications installer)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
MANIFEST="apple/Cargo.toml"
APP="$ROOT/apple/dist/AirDropd.app"
MACOS="$APP/Contents/MacOS"
RES="$APP/Contents/Resources"
DIST="$ROOT/dist"
PNG="$ROOT/assets/airdropd-icon.png"
PACKAGING="$ROOT/apple/packaging"

NATIVE_ARCH="$(uname -m)"
case "$NATIVE_ARCH" in
    arm64) NATIVE_TARGET="aarch64-apple-darwin"; ARCH_LABEL="arm64" ;;
    x86_64) NATIVE_TARGET="x86_64-apple-darwin"; ARCH_LABEL="x86_64" ;;
    *) echo "Unsupported macOS architecture: $NATIVE_ARCH" >&2; exit 1 ;;
esac

# Optional: MACOS_TARGET=aarch64-apple-darwin|x86_64-apple-darwin|universal
BUILD_MODE="${MACOS_TARGET:-$NATIVE_TARGET}"
if [[ "$BUILD_MODE" == "universal" ]]; then
    TARGETS=("aarch64-apple-darwin" "x86_64-apple-darwin")
    ARCH_LABEL="universal"
else
    TARGETS=("$BUILD_MODE")
    case "$BUILD_MODE" in
        aarch64-apple-darwin) ARCH_LABEL="arm64" ;;
        x86_64-apple-darwin) ARCH_LABEL="x86_64" ;;
        *) echo "Unknown MACOS_TARGET: $BUILD_MODE" >&2; exit 1 ;;
    esac
fi

echo "==> Building AirDropd for macOS ($ARCH_LABEL)..."

for target in "${TARGETS[@]}"; do
    if ! rustup target list --installed | grep -qx "$target"; then
        echo "==> Installing Rust target $target..."
        rustup target add "$target"
    fi
    echo "==> cargo build --release --target $target"
    cargo build --release --manifest-path "$MANIFEST" --target "$target"
done

mkdir -p "$DIST"
rm -rf "$APP"
mkdir -p "$MACOS" "$RES"

if [[ "${#TARGETS[@]}" -eq 2 ]]; then
    BIN="$MACOS/AirDropd"
    lipo -create \
        "$CARGO_TARGET_DIR/aarch64-apple-darwin/release/AirDropd" \
        "$CARGO_TARGET_DIR/x86_64-apple-darwin/release/AirDropd" \
        -output "$BIN"
else
    cp "$CARGO_TARGET_DIR/${TARGETS[0]}/release/AirDropd" "$MACOS/AirDropd"
fi
chmod +x "$MACOS/AirDropd"

cp "$PACKAGING/Info.plist" "$APP/Contents/Info.plist"

ICONSET="$(mktemp -d)/AppIcon.iconset"
mkdir -p "$ICONSET"
sips -z 16 16 "$PNG" --out "$ICONSET/icon_16x16.png" >/dev/null
sips -z 32 32 "$PNG" --out "$ICONSET/icon_16x16@2x.png" >/dev/null
sips -z 32 32 "$PNG" --out "$ICONSET/icon_32x32.png" >/dev/null
sips -z 64 64 "$PNG" --out "$ICONSET/icon_32x32@2x.png" >/dev/null
sips -z 128 128 "$PNG" --out "$ICONSET/icon_128x128.png" >/dev/null
sips -z 256 256 "$PNG" --out "$ICONSET/icon_128x128@2x.png" >/dev/null
sips -z 256 256 "$PNG" --out "$ICONSET/icon_256x256.png" >/dev/null
sips -z 512 512 "$PNG" --out "$ICONSET/icon_256x256@2x.png" >/dev/null
sips -z 512 512 "$PNG" --out "$ICONSET/icon_512x512.png" >/dev/null
sips -z 1024 1024 "$PNG" --out "$ICONSET/icon_512x512@2x.png" >/dev/null
iconutil -c icns "$ICONSET" -o "$RES/AppIcon.icns"

echo "==> Verifying binary..."
file "$MACOS/AirDropd"
ls -la "$MACOS/AirDropd"

ZIP="$DIST/AirDropd-macos-${ARCH_LABEL}.zip"
rm -f "$ZIP"
ditto -c -k --sequesterRsrc --keepParent "$APP" "$ZIP"
echo "Created $ZIP"

chmod +x "$PACKAGING/make-dmg.sh"
DMG="$DIST/AirDropd Setup.dmg"
rm -f "$DMG"
"$PACKAGING/make-dmg.sh" "$APP" "$DMG" "$PNG"

echo ""
echo "Done:"
echo "  $APP"
echo "  $ZIP"
echo "  $DMG"

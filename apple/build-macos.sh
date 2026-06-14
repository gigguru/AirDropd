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

chmod +x "$PACKAGING/generate-icons.sh"
"$PACKAGING/generate-icons.sh"

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
printf 'APPL????' > "$APP/Contents/PkgInfo"

# App bundle icon (Finder / Dock) — must match CFBundleIconFile in Info.plist.
"$PACKAGING/generate-icons.sh" "$RES/AppIcon.icns"

if [[ ! -f "$RES/AppIcon.icns" ]]; then
    echo "ERROR: AppIcon.icns was not created" >&2
    exit 1
fi
iconutil -c iconset "$RES/AppIcon.icns" -o "$(mktemp -d)/verify.iconset" >/dev/null
echo "==> Verified AppIcon.icns"

echo "==> Verifying binary..."
file "$MACOS/AirDropd"
ls -la "$MACOS/AirDropd" "$RES/AppIcon.icns"

ZIP="$DIST/AirDropd-macos-${ARCH_LABEL}.zip"
rm -f "$ZIP"
ditto -c -k --sequesterRsrc --keepParent "$APP" "$ZIP"
echo "Created $ZIP"

chmod +x "$PACKAGING/make-dmg.sh"
DMG="$DIST/AirDropd Setup.dmg"
rm -f "$DMG"
"$PACKAGING/make-dmg.sh" "$APP" "$DMG" "$ROOT/assets/airdropd-icon.png"

rm -rf "$DIST/AirDropd.app"
cp -R "$APP" "$DIST/AirDropd.app"

echo ""
echo "Done:"
echo "  $APP"
echo "  $DIST/AirDropd.app"
echo "  $ZIP"
echo "  $DMG"

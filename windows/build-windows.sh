#!/usr/bin/env bash
# Cross-compile AirDropd.exe for Windows from macOS/Linux.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

TARGET="${WINDOWS_TARGET:-x86_64-pc-windows-gnu}"
DIST="$ROOT/dist"

chmod +x "$ROOT/apple/packaging/generate-icons.sh"
"$ROOT/apple/packaging/generate-icons.sh"

echo "==> Building AirDropd for Windows ($TARGET)..."
export RUSTFLAGS="${RUSTFLAGS:--C target-feature=+crt-static}"
cargo build --release --manifest-path windows/Cargo.toml --target "$TARGET"

mkdir -p "$DIST"
EXE="$DIST/AirDropd.exe"
cp "$ROOT/target/$TARGET/release/AirDropd.exe" "$EXE"

ZIP="$DIST/AirDropd-windows-x86_64.zip"
rm -f "$ZIP"
ditto -c -k --keepParent "$EXE" "$ZIP"

echo ""
echo "Done:"
echo "  $EXE"
echo "  $ZIP"

#!/usr/bin/env bash
# Build platform icon files from assets/airdropd-icon.png
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PNG="$ROOT/assets/airdropd-icon.png"
ICO="$ROOT/assets/airdropd.ico"

usage() {
    echo "Usage: generate-icons.sh [icns-output-path]" >&2
    exit 1
}

[[ -f "$PNG" ]] || { echo "Missing $PNG" >&2; exit 1; }

echo "==> Syncing icons from $PNG"

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

if [[ $# -ge 1 ]]; then
    ICNS_OUT="$1"
    mkdir -p "$(dirname "$ICNS_OUT")"
    iconutil -c icns "$ICONSET" -o "$ICNS_OUT"
    echo "    wrote $ICNS_OUT"
fi

python3 - "$PNG" "$ICO" <<'PY'
import sys
from PIL import Image

png_path, ico_path = sys.argv[1], sys.argv[2]
img = Image.open(png_path).convert("RGBA")
sizes = [16, 24, 32, 48, 64, 128, 256]
img.save(ico_path, format="ICO", sizes=[(s, s) for s in sizes])
print(f"    wrote {ico_path}")
PY

echo "==> Icon sync complete"

#!/usr/bin/env bash
# Build all macOS distributables (arm64, x86_64, universal + DMG from universal).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

MACOS_TARGET=aarch64-apple-darwin ./apple/build-macos.sh
MACOS_TARGET=x86_64-apple-darwin ./apple/build-macos.sh
MACOS_TARGET=universal ./apple/build-macos.sh

echo ""
echo "All macOS builds complete in dist/"

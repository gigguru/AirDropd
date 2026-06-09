#!/usr/bin/env python3
"""Process AirDropd icon: white edges -> transparent, export PNG + ICO."""

from __future__ import annotations

from collections import deque
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "assets" / "airdropd-icon-source.png"
OUT_PNG = ROOT / "assets" / "airdropd-icon.png"
OUT_ICO = ROOT / "assets" / "airdropd.ico"


def is_bg(r: int, g: int, b: int, a: int, threshold: int = 242) -> bool:
    return a > 0 and r >= threshold and g >= threshold and b >= threshold


def main() -> None:
    if not SRC.exists():
        raise SystemExit(f"Missing source image: {SRC}")

    img = Image.open(SRC).convert("RGBA")
    pixels = img.load()
    w, h = img.size

    visited = [[False] * w for _ in range(h)]
    q: deque[tuple[int, int]] = deque()

    for x in range(w):
        for y in (0, h - 1):
            if not visited[y][x] and is_bg(*pixels[x, y]):
                visited[y][x] = True
                q.append((x, y))
    for y in range(h):
        for x in (0, w - 1):
            if not visited[y][x] and is_bg(*pixels[x, y]):
                visited[y][x] = True
                q.append((x, y))

    while q:
        x, y = q.popleft()
        pr, pg, pb, _ = pixels[x, y]
        pixels[x, y] = (pr, pg, pb, 0)
        for nx, ny in ((x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)):
            if 0 <= nx < w and 0 <= ny < h and not visited[ny][nx] and is_bg(*pixels[nx, ny]):
                visited[ny][nx] = True
                q.append((nx, ny))

    for y in range(h):
        for x in range(w):
            r, g, b, a = pixels[x, y]
            if a == 0:
                continue
            if r >= 228 and g >= 228 and b >= 228:
                edge = min(255 - r, 255 - g, 255 - b)
                new_a = int(a * min(1.0, edge / 18.0))
                pixels[x, y] = (r, g, b, 0 if new_a < 16 else new_a)

    img.save(OUT_PNG)
    sizes = [16, 24, 32, 48, 64, 128, 256]
    img.resize((256, 256), Image.Resampling.LANCZOS).save(
        OUT_ICO, format="ICO", sizes=[(s, s) for s in sizes]
    )
    print(f"Wrote {OUT_PNG} and {OUT_ICO}")


if __name__ == "__main__":
    main()

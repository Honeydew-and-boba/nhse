#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Makes the Switch app's icon and in-app logo masks from assets/switch-logo.png.

    python3 switch/tools/make-logo.py

The logo is a white glyph on a transparent background: a rounded frame, a palm tree on an island,
and a pencil with sparkles. This script writes
  - switch/icon.jpg: the 256x256 icon of the homebrew menu (the glyph on the app's green);
  - crates/nhse-switch/assets/logo.mask: the whole glyph, and
  - crates/nhse-switch/assets/palm.mask: the palm tree and island only,
as alpha masks the app tints in its theme colors (format: u16 width, u16 height, little
endian, then width*height bytes of alpha). Needs Pillow and NumPy.
"""
from collections import deque
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "assets/switch-logo.png"
GREEN = (35, 162, 111)  # the desktop app's accent (--accent)
MASK_HEIGHT = 128  # the app scales masks down from this height


def components(mask):
    """Connected shapes of a boolean mask: a list of (pixel count, bbox, label), and the labels."""
    h, w = mask.shape
    labels = np.zeros((h, w), np.int32)
    out = []
    for y, x in zip(*np.nonzero(mask)):
        if labels[y, x]:
            continue
        n = len(out) + 1
        labels[y, x] = n
        queue = deque([(y, x)])
        count, x0, y0, x1, y1 = 0, x, y, x, y
        while queue:
            cy, cx = queue.popleft()
            count += 1
            x0, x1, y0, y1 = min(x0, cx), max(x1, cx), min(y0, cy), max(y1, cy)
            for ny, nx in ((cy + 1, cx), (cy - 1, cx), (cy, cx + 1), (cy, cx - 1)):
                if 0 <= ny < h and 0 <= nx < w and mask[ny, nx] and not labels[ny, nx]:
                    labels[ny, nx] = n
                    queue.append((ny, nx))
        out.append((count, (x0, y0, x1 + 1, y1 + 1), n))
    return out, labels


def crop(alpha, keep):
    """The alpha of the kept pixels, cropped to them."""
    a = np.where(keep, alpha, 0).astype(np.uint8)
    ys, xs = np.nonzero(a)
    return a[ys.min() : ys.max() + 1, xs.min() : xs.max() + 1]


def write_mask(alpha, path):
    img = Image.fromarray(alpha, "L")
    w = round(img.width * MASK_HEIGHT / img.height)
    img = img.resize((w, MASK_HEIGHT), Image.LANCZOS)
    path.write_bytes(w.to_bytes(2, "little") + MASK_HEIGHT.to_bytes(2, "little") + img.tobytes())
    print(f"{path.relative_to(ROOT)}: {w}x{MASK_HEIGHT}")


def main():
    img = Image.open(SOURCE).convert("RGBA")
    alpha = np.array(img.getchannel("A"))
    solid = alpha >= 128
    comps, labels = components(solid)
    # drop specks; the faint, half transparent noise around the glyph goes with the threshold
    shapes = [c for c in comps if c[0] >= 0.001 * solid.size]
    # the frame is the largest shape; the palm tree's shapes lie entirely inside it (the pencil
    # crosses the frame's edge and the sparkles are outside)
    shapes.sort(key=lambda c: -c[0])
    fx0, fy0, fx1, fy1 = shapes[0][1]
    palm = [c for c in shapes[1:] if c[1][0] > fx0 and c[1][1] > fy0 and c[1][2] < fx1 and c[1][3] < fy1]
    clean = np.where(solid, 255, 0).astype(np.uint8)
    every = np.isin(labels, [c[2] for c in shapes])
    write_mask(crop(clean, every), ROOT / "crates/nhse-switch/assets/logo.mask")
    write_mask(crop(clean, np.isin(labels, [c[2] for c in palm])), ROOT / "crates/nhse-switch/assets/palm.mask")

    # homebrew menu icon: the whole glyph, centered on green
    glyph = Image.fromarray(crop(clean, every), "L")
    size, box = 256, 200
    scale = box / max(glyph.size)
    glyph = glyph.resize((round(glyph.width * scale), round(glyph.height * scale)), Image.LANCZOS)
    icon = Image.new("RGB", (size, size), GREEN)
    white = Image.new("RGB", glyph.size, (255, 255, 255))
    icon.paste(white, ((size - glyph.width) // 2, (size - glyph.height) // 2), glyph)
    icon.save(ROOT / "switch/icon.jpg", quality=95)
    print("switch/icon.jpg: 256x256")


if __name__ == "__main__":
    main()

"""Assemble PNG frames (from record-demo.mjs) into an optimised GIF.

Usage: python scripts/make-gif.py <framesDir> <out.gif> [width] [ms_per_frame]
"""

import sys
from pathlib import Path

from PIL import Image

frames_dir = Path(sys.argv[1])
out = Path(sys.argv[2])
width = int(sys.argv[3]) if len(sys.argv) > 3 else 760
ms = int(sys.argv[4]) if len(sys.argv) > 4 else 110

paths = sorted(frames_dir.glob("f*.png"))
if not paths:
    sys.exit(f"no frames in {frames_dir}")

frames = []
for p in paths:
    im = Image.open(p).convert("RGB")
    h = round(im.height * width / im.width)
    frames.append(im.resize((width, h), Image.LANCZOS))

# One shared palette keeps colours stable between frames and the file small.
palette = frames[len(frames) // 2].quantize(colors=200, method=Image.Quantize.MEDIANCUT)
quantized = [f.quantize(palette=palette, dither=Image.Dither.FLOYDSTEINBERG) for f in frames]
durations = [ms] * len(quantized)
durations[-1] = 1800  # hold the touchdown card
quantized[0].save(
    out, save_all=True, append_images=quantized[1:], duration=durations, loop=0, optimize=True
)
print(f"wrote {out} ({len(quantized)} frames, {out.stat().st_size / 1e6:.1f} MB)")

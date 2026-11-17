"""Atlante dei ritratti (PixelLab, 64x64) per schede e roster.

In `pixellab/portraits/<popolo>_<classe>.png` ci sono i ritratti generati.
L'atlante ha una colonna per classe e due righe per popolo (squadra 0 e 1):
riga popolo * 2 + squadra. Per la squadra 1 il blu (vestiti e sfondo) ruota
verso il rosso mantenendo luce e saturazione. Dove manca il ritratto di un popolo si usa quello umano.

  python3 tools/pixelart/pixellab_portraits.py   # scrive client/assets/override/portraits.png
"""

from __future__ import annotations

import colorsys
import sys
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).parent))

import artkit  # noqa: E402
from pixellab_import import PEOPLE  # noqa: E402

HERE = Path(__file__).parent
SRC = HERE / "pixellab" / "portraits"
OUT = HERE.parents[1] / "client" / "assets" / "override" / "portraits.png"
P = 64


def red_team(im: Image.Image) -> Image.Image:
    """Squadra 1: il blu saturo dei vestiti ruota verso il rosso."""
    out = im.copy()
    px = out.load()
    for y in range(out.height):
        for x in range(out.width):
            r, g, b, a = px[x, y]
            if not a:
                continue
            h, s, v = colorsys.rgb_to_hsv(r / 255, g / 255, b / 255)
            if 0.53 <= h <= 0.72 and s >= 0.4:
                nr, ng, nb = colorsys.hsv_to_rgb(0.995, min(1.0, s * 1.05), v)
                px[x, y] = (int(nr * 255), int(ng * 255), int(nb * 255), a)
    return out


def main() -> None:
    atlas = Image.new("RGBA", (P * len(artkit.CLASS_ORDER), P * 2 * len(PEOPLE)), (0, 0, 0, 0))
    found = 0
    for c, cls in enumerate(artkit.CLASS_ORDER):
        for p, people in enumerate(PEOPLE):
            src = SRC / f"{people}_{cls}.png"
            if src.exists():
                found += 1
            else:
                src = SRC / f"human_{cls}.png"
            if not src.exists():
                continue
            im = Image.open(src).convert("RGBA")
            atlas.alpha_composite(im, (c * P, p * 2 * P))
            atlas.alpha_composite(red_team(im), (c * P, (p * 2 + 1) * P))
    OUT.parent.mkdir(parents=True, exist_ok=True)
    atlas.save(OUT)
    print(OUT.relative_to(HERE.parents[1]), atlas.size, "ritratti:", found)


if __name__ == "__main__":
    main()

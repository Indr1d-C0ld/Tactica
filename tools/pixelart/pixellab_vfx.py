"""Atlante degli effetti delle abilità dagli effetti PixelLab (vfx_workbench).

In `pixellab/vfx/` ci sono gli spritesheet esportati (<effetto>.png + .json,
formato TexturePacker). Ogni effetto diventa una riga di 8 fotogrammi da 64x64,
centrati sull'origine dell'effetto; la riga "arrow" contiene il proiettile nel
primo fotogramma, rivolto a destra.

  python3 tools/pixelart/pixellab_vfx.py     # scrive client/assets/override/vfx.png
"""

from __future__ import annotations

import json
from pathlib import Path

from PIL import Image

HERE = Path(__file__).parent
SRC = HERE / "pixellab" / "vfx"
OUT = HERE.parents[1] / "client" / "assets" / "override" / "vfx.png"
CELL = 64
FRAMES = 8
# Stesso ordine di VFX_ROWS in board3d.gd.
ROWS = ["slash", "arrow", "impact", "fire", "ice", "thunder", "holy", "heal", "buff", "debuff", "poison", "drain"]


def frames(key: str) -> list[Image.Image]:
    sheet = Image.open(SRC / f"{key}.png").convert("RGBA")
    out = []
    for f in json.load(open(SRC / f"{key}.json"))["frames"]:
        r = f["frame"]
        out.append(sheet.crop((r["x"], r["y"], r["x"] + r["w"], r["y"] + r["h"])))
    return out


def main() -> None:
    atlas = Image.new("RGBA", (CELL * FRAMES, CELL * len(ROWS)), (0, 0, 0, 0))
    for row, key in enumerate(ROWS):
        fr = frames(key)
        if key == "arrow":
            # Il fotogramma con la freccia intera, ritagliata e centrata.
            best = max(fr, key=lambda im: sum(1 for p in im.getdata() if p[3]))
            fr = [best.crop(best.getbbox())]
        for col, im in enumerate(fr[:FRAMES]):
            if im.width > CELL or im.height > CELL:
                im = im.crop(((im.width - CELL) // 2 if im.width > CELL else 0,
                              (im.height - CELL) // 2 if im.height > CELL else 0,
                              min(im.width, (im.width + CELL) // 2), min(im.height, (im.height + CELL) // 2)))
            atlas.alpha_composite(im, (col * CELL + (CELL - im.width) // 2, row * CELL + (CELL - im.height) // 2))
    OUT.parent.mkdir(parents=True, exist_ok=True)
    atlas.save(OUT)
    print(OUT.relative_to(HERE.parents[1]), atlas.size)


if __name__ == "__main__":
    main()

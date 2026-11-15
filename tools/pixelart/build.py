"""Genera gli atlanti di pixel-art del client in client/assets/.

Uso: python3 tools/pixelart/build.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).parent))

import palette as P  # noqa: E402
import tiles  # noqa: E402
import tiles3d  # noqa: E402
import units  # noqa: E402
from pixlib import hexc  # noqa: E402

OUT = Path(__file__).resolve().parents[2] / "client" / "assets"
CLASS_ORDER = ["vanguard", "scout", "arcanist", "mender", "runeblade", "warden"]
HAIR = list(P.HAIR)
SKIN = list(P.SKIN)


def build_units():
    rows = []
    for cls in CLASS_ORDER:
        for team in (0, 1):
            for hair in HAIR:
                for skin in SKIN:
                    rows.append(units.sheet_row(cls, team, hair, skin))
    w = rows[0].width
    sheet = Image.new("RGBA", (w, units.H * len(rows)), (0, 0, 0, 0))
    for i, r in enumerate(rows):
        sheet.paste(r, (0, i * units.H))
    sheet.save(OUT / "units.png")
    meta = {
        "cell": units.W,
        "frames": units.FRAMES,
        "classes": CLASS_ORDER,
        "teams": 2,
        "hair": HAIR,
        "skin": SKIN,
        "row_formula": "((class * teams + team) * len(hair) + hair) * len(skin) + skin",
    }
    (OUT / "units.json").write_text(json.dumps(meta, indent=2, ensure_ascii=False))
    print("units.png", sheet.size)


def frame_texture(size: int, border: int, fill, light, dark, outline=(27, 22, 38, 255)) -> Image.Image:
    img = Image.new("RGBA", (size, size), fill)
    p = img.load()
    for y in range(size):
        for x in range(size):
            d = min(x, y, size - 1 - x, size - 1 - y)
            if d == 0:
                p[x, y] = outline
            elif d < border:
                p[x, y] = light if (x < size - 1 - y and d == min(x, y)) else dark
    # Borchie agli angoli.
    for cx, cy in ((1, 1), (size - 2, 1), (1, size - 2), (size - 2, size - 2)):
        p[cx, cy] = (255, 240, 180, 255)
    return img


def build_ui():
    gold_l, gold_d = hexc("#e9c26e"), hexc("#8a5a24")
    frame_texture(16, 4, hexc("#1c2742"), gold_l, gold_d).save(OUT / "ui_window.png")
    frame_texture(12, 3, hexc("#2c3d63"), gold_l, gold_d).save(OUT / "ui_button.png")
    frame_texture(12, 3, hexc("#3e5a8e"), hexc("#fff0b0"), gold_d).save(OUT / "ui_button_hover.png")
    frame_texture(12, 3, hexc("#18223a"), gold_d, gold_l).save(OUT / "ui_button_pressed.png")
    frame_texture(12, 3, hexc("#262a33"), hexc("#7a7468"), hexc("#4a4640")).save(OUT / "ui_button_disabled.png")
    frame_texture(12, 3, hexc("#121a2c"), gold_d, hexc("#5a3c18")).save(OUT / "ui_field.png")
    print("ui_*.png")


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    tiles.atlas().save(OUT / "terrain.png")
    tiles3d.atlas().save(OUT / "terrain3d.png")
    print("terrain.png, terrain3d.png")
    build_units()
    build_ui()


if __name__ == "__main__":
    main()

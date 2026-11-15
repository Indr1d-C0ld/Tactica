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
import font  # noqa: E402
import vfx  # noqa: E402
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


def window_texture(size: int, fill_top, fill_bottom, rim, rim_dark, outline=(18, 14, 30, 255), radius=2) -> Image.Image:
    """Cornice a 1x: contorno scuro con angoli arrotondati, bordo chiaro in alto
    a sinistra e scuro in basso a destra, interno a bande con retino."""
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    p = img.load()
    for y in range(size):
        for x in range(size):
            # Angoli arrotondati.
            cx = min(x, size - 1 - x)
            cy = min(y, size - 1 - y)
            if cx + cy < radius - 1:
                continue
            d = min(cx, cy)
            if d == 0 or (cx + cy == radius - 1):
                p[x, y] = outline
            elif d == 1:
                p[x, y] = rim if (x <= y and x < size - 1 - y) or y < size // 2 and x < size - 1 - y else rim_dark
                if x == size - 2 or y == size - 2:
                    p[x, y] = rim_dark
                if x == 1 or y == 1:
                    p[x, y] = rim
            elif d == 2:
                p[x, y] = outline
            else:
                t = (y - 3) / max(1, size - 7)
                c = tuple(int(fill_top[k] * (1 - t) + fill_bottom[k] * t) for k in range(3)) + (255,)
                p[x, y] = c
    return img


def scaled(img: Image.Image, k: int = 2) -> Image.Image:
    return img.resize((img.width * k, img.height * k), Image.NEAREST)


def build_ui():
    cream, cream_d = hexc("#f2e3b8"), hexc("#a3895a")
    gold_l, gold_d = hexc("#f6d27a"), hexc("#8a5a24")
    scaled(window_texture(16, hexc("#2b3a66"), hexc("#18213f"), cream, cream_d)).save(OUT / "ui_window.png")
    scaled(window_texture(12, hexc("#3a4f86"), hexc("#25335c"), cream, cream_d)).save(OUT / "ui_button.png")
    scaled(window_texture(12, hexc("#5470b4"), hexc("#34487f"), (255, 250, 220, 255), gold_l)).save(OUT / "ui_button_hover.png")
    scaled(window_texture(12, hexc("#18223f"), hexc("#22305a"), cream_d, cream)).save(OUT / "ui_button_pressed.png")
    scaled(window_texture(12, hexc("#30323c"), hexc("#25272f"), hexc("#7a7468"), hexc("#4a4640"))).save(OUT / "ui_button_disabled.png")
    scaled(window_texture(12, hexc("#101830"), hexc("#141e3a"), gold_d, gold_l)).save(OUT / "ui_field.png")
    print("ui_*.png")


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    tiles.atlas().save(OUT / "terrain.png")
    tiles3d.atlas().save(OUT / "terrain3d.png")
    tiles3d.backdrop().save(OUT / "backdrop.png")
    tiles3d.worldmap().save(OUT / "worldmap.png")
    vfx.atlas().save(OUT / "vfx.png")
    print("terrain.png, terrain3d.png")
    build_units()
    build_ui()
    font.build(OUT / "fonts")


if __name__ == "__main__":
    main()

"""Importa i personaggi generati con PixelLab nel foglio base degli sprite.

Per ogni classe c'è una cartella `tools/pixelart/pixellab/<classe>/pack/`
(l'archivio scaricato da PixelLab, estratto) con:

  Idle/rotations/south-east.png, north-east.png      viste a riposo
  Idle/animations/walk/{south-east,north-east}/*.png camminata
  Idle/animations/strike/south-east/*.png            azione
  Idle/animations/ko/south-east/*.png                caduta

Lo script compone la riga della classe nel formato di units_base.png
(11 fotogrammi da 32x32, piedi in (16, 30)) e trasforma il blu dei vestiti nei
colori chiave della squadra, così artkit può ricolorarlo per le due squadre.
Le classi senza cartella restano quelle generate da units.py.

  python3 tools/pixelart/pixellab_import.py     # scrive client/assets/override/units.png
"""

from __future__ import annotations

import colorsys
import sys
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).parent))

import artkit  # noqa: E402

HERE = Path(__file__).parent
SRC = HERE / "pixellab"
CELL = 32
FOOT = (16, 30)

# Fotogrammi scelti dalle animazioni (indice nel clip).
WALK_PICK = (1, 4)
STRIKE_PICK = 3


def load(path: Path) -> Image.Image:
    return Image.open(path).convert("RGBA")


def frames(pack: Path, anim: str, direction: str) -> list[Image.Image]:
    return [load(p) for p in sorted((pack / "Idle" / "animations" / anim / direction).glob("*.png"))]


def to_cell(im: Image.Image) -> Image.Image:
    """Ritaglia al centro una cella 32x32 (le animazioni hanno tele più grandi)."""
    if im.size == (CELL, CELL):
        return im
    ox, oy = (im.width - CELL) // 2, (im.height - CELL) // 2
    return im.crop((ox, oy, ox + CELL, oy + CELL))


def foot_offset(ref: Image.Image) -> tuple[int, int]:
    """Spostamento che porta la base del personaggio di riferimento in FOOT."""
    px = ref.load()
    bottom = max(y for y in range(ref.height) for x in range(ref.width) if px[x, y][3])
    xs = [x for y in range(bottom - 3, bottom + 1) for x in range(ref.width) if px[x, y][3]]
    cx = (min(xs) + max(xs) + 1) // 2
    return FOOT[0] - cx, FOOT[1] - bottom


def shift(im: Image.Image, dx: int, dy: int) -> Image.Image:
    out = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
    out.alpha_composite(im, (dx, dy)) if dx >= 0 and dy >= 0 else out.paste(im, (dx, dy), im)
    return out


def breathe(im: Image.Image) -> Image.Image:
    """Secondo fotogramma del riposo: busto e testa scendono di un pixel."""
    bbox = im.getbbox()
    if not bbox:
        return im
    waist = bbox[1] + (bbox[3] - bbox[1]) * 3 // 5
    out = Image.new("RGBA", im.size, (0, 0, 0, 0))
    out.paste(im.crop((0, waist, CELL, CELL)), (0, waist))
    top = im.crop((0, 0, CELL, waist))
    out.alpha_composite(top, (0, 1))
    return out


def key_team(im: Image.Image) -> Image.Image:
    """Blu dei vestiti -> colori chiave della squadra, per luminosità."""
    px = im.load()
    blues = []
    for y in range(im.height):
        for x in range(im.width):
            r, g, b, a = px[x, y]
            if not a:
                continue
            # Stoffa blu molto satura; l'acciaio è bluastro ma poco saturo.
            h, s, v = colorsys.rgb_to_hsv(r / 255, g / 255, b / 255)
            if 0.55 <= h <= 0.70 and s >= 0.55 and v >= 0.18:
                blues.append((x, y, v))
    if not blues:
        return im
    ls = sorted(l for _, _, l in blues)
    cuts = [ls[len(ls) * q // 4] for q in (1, 2, 3)]
    keys = artkit.KEY_TEAM
    out = im.copy()
    po = out.load()
    for x, y, l in blues:
        i = sum(l >= c for c in cuts)
        po[x, y] = keys[i][:3] + (px[x, y][3],)
    return out


def class_row(pack: Path) -> Image.Image:
    rot = pack / "Idle" / "rotations"
    se, ne = load(rot / "south-east.png"), load(rot / "north-east.png")
    dx, dy = foot_offset(se)
    place = lambda im: shift(to_cell(im), dx, dy)  # noqa: E731

    walk_se, walk_ne = frames(pack, "walk", "south-east"), frames(pack, "walk", "north-east")
    strike = frames(pack, "strike", "south-east")
    ko = frames(pack, "ko", "south-east")

    front, back = place(se), place(ne)
    cells = [
        front, breathe(front),
        back, breathe(back),
        place(strike[min(STRIKE_PICK, len(strike) - 1)]) if strike else front,
        shift(back, 1, -1),
        place(ko[-1]) if ko else front,
        *(place(walk_se[i]) for i in WALK_PICK),
        *(place(walk_ne[i]) for i in WALK_PICK),
    ]
    row = Image.new("RGBA", (CELL * len(cells), CELL), (0, 0, 0, 0))
    for i, c in enumerate(cells):
        row.alpha_composite(key_team(c), (i * CELL, 0))
    return row


def main() -> None:
    base = artkit.base_sheet()
    done = []
    for i, cls in enumerate(artkit.CLASS_ORDER):
        pack = SRC / cls / "pack"
        if (pack / "Idle" / "rotations" / "south-east.png").exists():
            base.paste(Image.new("RGBA", (base.width, CELL), (0, 0, 0, 0)), (0, i * CELL))
            base.alpha_composite(class_row(pack), (0, i * CELL))
            done.append(cls)
    out_dir = HERE / "templates"
    base.save(out_dir / "units_base_pixellab.png")
    dest = HERE.parents[1] / "client" / "assets" / "override"
    dest.mkdir(parents=True, exist_ok=True)
    artkit.expand(base).save(dest / "units.png")
    print("classi da PixelLab:", ", ".join(done) or "nessuna")
    print("client/assets/override/units.png")


if __name__ == "__main__":
    main()

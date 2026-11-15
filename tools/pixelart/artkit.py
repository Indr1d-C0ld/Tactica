"""Kit per il pixel artist: foglio base dei personaggi e ricolorazione.

Un artista disegna UN solo foglio base (6 classi x 11 fotogrammi, celle 32x32)
usando colori chiave riservati per squadra, capelli e pelle. Questo script
genera da quel foglio tutte le varianti dell'atlante units.png
(classe x squadra x capelli x carnagione), sostituendo i colori chiave con le
palette del gioco.

  python3 tools/pixelart/artkit.py template   # scrive il foglio base generato e il modello con le guide
  python3 tools/pixelart/artkit.py build FILE # da un foglio base disegnato a mano scrive client/assets/units.png
"""

from __future__ import annotations

import sys
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).parent))

import palette as P  # noqa: E402
import units  # noqa: E402
from pixlib import Mat, darken, ramp  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = Path(__file__).parent / "templates"
CLASS_ORDER = ["vanguard", "scout", "arcanist", "mender", "runeblade", "warden"]

# Colori chiave (dal più scuro al più chiaro): non compaiono altrove.
KEY_TEAM = ramp("#400040", "#800080", "#c000c0", "#ff40ff")
KEY_HAIR = ramp("#004040", "#008080", "#00c0c0", "#40ffff")
KEY_SKIN = ramp("#404000", "#808000", "#c0c000", "#ffff40")


def key_variants(key: list) -> list:
    """Toni chiave più le due varianti di contorno generate dal disegnatore."""
    return list(key) + [darken(key[0], 0.55), darken(key[0], 0.78)]


def target_variants(r: list) -> list:
    return list(r) + [darken(r[0], 0.55), darken(r[0], 0.78)]


def base_sheet() -> Image.Image:
    """Foglio base generato con i colori chiave (punto di partenza per l'artista)."""
    orig = (P.TEAM, P.HAIR, P.SKIN)
    try:
        P.TEAM = [KEY_TEAM, KEY_TEAM]
        P.HAIR = {"chiave": KEY_HAIR}
        P.SKIN = {"chiave": KEY_SKIN}
        rows = [units.sheet_row(c, 0, "chiave", "chiave") for c in CLASS_ORDER]
    finally:
        P.TEAM, P.HAIR, P.SKIN = orig
    sheet = Image.new("RGBA", (rows[0].width, units.H * len(rows)), (0, 0, 0, 0))
    for i, r in enumerate(rows):
        sheet.paste(r, (0, i * units.H))
    return sheet


def guides(sheet: Image.Image) -> Image.Image:
    """Copia ingrandita 4x con griglia, punto d'appoggio dei piedi e numeri dei fotogrammi."""
    k = 4
    big = Image.new("RGBA", (sheet.width * k, sheet.height * k + 24), (40, 44, 58, 255))
    bg = Image.new("RGBA", sheet.size, (0, 0, 0, 0))
    for y in range(sheet.height):
        for x in range(sheet.width):
            cx, cy = x // units.W, y // units.H
            bg.putpixel((x, y), (54, 60, 78, 255) if (cx + cy) % 2 else (62, 68, 88, 255))
    bg.alpha_composite(sheet)
    big.paste(bg.resize((sheet.width * k, sheet.height * k), Image.NEAREST), (0, 24))
    px = big.load()
    for x in range(big.width):
        for y in range(24, big.height, units.H * k):
            px[x, y] = (255, 210, 90, 255)
    for y in range(24, big.height):
        for x in range(0, big.width, units.W * k):
            px[x, y] = (255, 210, 90, 255)
    # Piedi: (16, 30) in ogni cella.
    for cy in range(sheet.height // units.H):
        for cx in range(sheet.width // units.W):
            fx, fy = (cx * units.W + 16) * k, 24 + (cy * units.H + 30) * k
            for d in range(-3, 4):
                px[fx + d, fy] = (255, 60, 60, 255)
                px[fx, fy + d] = (255, 60, 60, 255)
    # Numeri dei fotogrammi in testa (font del progetto).
    import font

    for i in range(len(units.FRAMES)):
        x0 = i * units.W * k + 6
        for j, ch in enumerate(str(i)):
            rows = font.cell_rows(font.G[ch])
            for yy, row in enumerate(rows):
                for xx, v in enumerate(row):
                    if v == "#":
                        for d1 in range(2):
                            for d2 in range(2):
                                px[x0 + j * 14 + xx * 2 + d1, 2 + yy * 2 + d2] = (255, 230, 170, 255)
    return big


def expand(base: Image.Image) -> Image.Image:
    """Dal foglio base all'atlante completo, con l'ordine di righe del gioco."""
    hair_names, skin_names = list(P.HAIR), list(P.SKIN)
    rows = []
    for ci in range(len(CLASS_ORDER)):
        cls_row = base.crop((0, ci * units.H, base.width, (ci + 1) * units.H))
        for team in (0, 1):
            for hair in hair_names:
                for skin in skin_names:
                    mapping = {}
                    for keys, target in ((KEY_TEAM, P.TEAM[team]), (KEY_HAIR, P.HAIR[hair]), (KEY_SKIN, P.SKIN[skin])):
                        for a, b in zip(key_variants(keys), target_variants(target)):
                            mapping[a[:3]] = b
                    row = cls_row.copy()
                    px = row.load()
                    for y in range(row.height):
                        for x in range(row.width):
                            c = px[x, y]
                            if c[3] and c[:3] in mapping:
                                px[x, y] = mapping[c[:3]][:3] + (c[3],)
                    rows.append(row)
    out = Image.new("RGBA", (base.width, units.H * len(rows)), (0, 0, 0, 0))
    for i, r in enumerate(rows):
        out.paste(r, (0, i * units.H))
    return out


def main():
    cmd = sys.argv[1] if len(sys.argv) > 1 else "template"
    if cmd == "template":
        OUT_DIR.mkdir(parents=True, exist_ok=True)
        base = base_sheet()
        base.save(OUT_DIR / "units_base.png")
        guides(base).save(OUT_DIR / "units_base_guide.png")
        print("templates/units_base.png", base.size)
    elif cmd == "build":
        base = Image.open(sys.argv[2]).convert("RGBA")
        assert base.width == units.W * len(units.FRAMES) and base.height == units.H * len(CLASS_ORDER), (
            f"il foglio base deve essere {units.W * len(units.FRAMES)}x{units.H * len(CLASS_ORDER)}"
        )
        dest = ROOT / "client" / "assets" / "override"
        dest.mkdir(parents=True, exist_ok=True)
        expand(base).save(dest / "units.png")
        print("client/assets/override/units.png")
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
    _ = Mat

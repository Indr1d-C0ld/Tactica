"""Atlante dei terreni dalle tessere PixelLab (celle 32x32).

Sorgenti in tools/pixelart/pixellab/terrain/:
  tops/        sommità (create_tiles_pro, vista dall'alto)
  sides/       fianchi verticali, normali e con il bordo che ricade
  lava_sides/  fianchi di basalto e lava
  decor/       decorazioni con sfondo trasparente (Pixen)

Formato di terrain3d.png (come il generato, ma con celle da 32):
  righe 0-5: erba, roccia, sabbia, acqua, lava, boscaglia
  colonne 0-2: sommità (varianti; per acqua e lava, tre fotogrammi)
  colonna 3: fianco; colonna 4: fianco del livello più alto
  riga 6: alberello, cespuglio, sassi, ciuffo d'erba

  python3 tools/pixelart/pixellab_terrain.py   # scrive client/assets/override/terrain3d.png
"""

from pathlib import Path

from PIL import Image

HERE = Path(__file__).parent / "pixellab" / "terrain"
OUT = Path(__file__).resolve().parents[2] / "client" / "assets" / "override" / "terrain3d.png"
C = 32


def load(sub: str, i: int) -> Image.Image:
    return Image.open(HERE / sub / f"tile_{i}.png").convert("RGBA").resize((C, C), Image.NEAREST)


# Righe di base da tagliare per decorazione (tavoletta sotto i sassi, pozza sotto il ciuffo).
BASE_ROWS = {2: 3, 3: 4}


def decor(i: int) -> Image.Image:
    im = Image.open(HERE / "decor" / f"decor_{i}.png").convert("RGBA").resize((C, C), Image.NEAREST)
    im = strip_base(im)
    cut = BASE_ROWS.get(i, 0)
    if cut:
        bbox = im.getbbox()
        if bbox:
            px = im.load()
            for y in range(bbox[3] - cut, bbox[3]):
                for x in range(C):
                    px[x, y] = (0, 0, 0, 0)
    return im


def strip_base(im: Image.Image) -> Image.Image:
    """Toglie le basi indesiderate (tavolette, pozze) in fondo alle decorazioni:
    righe basse in cui prevalgono marroni o azzurri."""
    import colorsys

    im = im.copy()
    px = im.load()
    for y in range(C - 1, C // 2, -1):
        cols = [px[x, y] for x in range(C) if px[x, y][3]]
        if not cols:
            continue
        def kind(c):
            h, s, v = colorsys.rgb_to_hsv(*(t / 255 for t in c[:3]))
            if s > 0.3 and (0.04 < h < 0.13):
                return "brown"
            if s > 0.25 and 0.45 < h < 0.7:
                return "blue"
            return "other"
        k = [kind(c) for c in cols]
        if k.count("brown") + k.count("blue") > len(k) // 2:
            for x in range(C):
                px[x, y] = (0, 0, 0, 0)
        else:
            break
    return im


def no_border(im: Image.Image, ring: int = 2) -> Image.Image:
    """Le tessere hanno un bordo scuro: lo sostituisce con i pixel interni
    (a specchio), così le caselle vicine si fondono senza griglia marcata."""
    im = im.copy()
    px = im.load()
    for y in range(C):
        for x in range(C):
            sx = x if ring <= x < C - ring else (2 * ring - 1 - x if x < ring else 2 * (C - ring) - 1 - x)
            sy = y if ring <= y < C - ring else (2 * ring - 1 - y if y < ring else 2 * (C - ring) - 1 - y)
            if (sx, sy) != (x, y):
                px[x, y] = px[sx, sy]
    return im


def rotations(im: Image.Image) -> list[Image.Image]:
    return [im, im.rotate(90), im.rotate(180)]


def scroll_frames(im: Image.Image) -> list[Image.Image]:
    """Tre fotogrammi: la tessera che scorre di un terzo ogni volta (con riavvolgimento)."""
    out = []
    for k in range(3):
        dx = k * C // 3
        f = Image.new("RGBA", (C, C))
        f.paste(im.crop((dx, 0, C, C)), (0, 0))
        f.paste(im.crop((0, 0, dx, C)), (C - dx, 0))
        out.append(f)
    return out


def fix_top_edge(im: Image.Image) -> Image.Image:
    """Toglie lo sfondo chiaro rimasto sul bordo alto dei fianchi (copia il pixel sotto)."""
    im = im.copy()
    px = im.load()
    for y in range(7, -1, -1):
        for x in range(C):
            r, g, b, a = px[x, y]
            if a == 0 or (r > 215 and g > 215 and b > 215):
                px[x, y] = px[x, y + 1]
    return im


def edge(im: Image.Image) -> Image.Image:
    """Filo sottile sui lati destro e basso della sommità: fra caselle vicine
    resta una linea di 1 pixel, che definisce la griglia senza appesantirla."""
    im = im.copy()
    px = im.load()
    for i in range(C):
        for x, y, k in ((C - 1, i, 0.72), (i, C - 1, 0.72), (0, i, 1.08), (i, 0, 1.08)):
            r, g, b, a = px[x, y]
            px[x, y] = (min(255, int(r * k)), min(255, int(g * k)), min(255, int(b * k)), a)
    return im


def opaque(im: Image.Image) -> Image.Image:
    """Le sommità e i fianchi devono essere pieni (niente trasparenze)."""
    bg = Image.new("RGBA", im.size, im.resize((1, 1), Image.BILINEAR).getpixel((0, 0))[:3] + (255,))
    bg.alpha_composite(im)
    return bg


def main() -> None:
    tops = {i: load("tops", i) for i in range(16)}
    rows = [
        # (sommità x3, fianco, fianco alto)
        ([tops[0], tops[1], tops[2]], load("sides", 1), load("sides", 3)),          # erba
        (rotations(tops[3]), load("sides", 4), load("sides", 5)),                  # roccia
        (rotations(tops[8]), load("sides", 6), load("sides", 7)),                  # sabbia
        (scroll_frames(no_border(tops[13])), load("sides", 10), load("sides", 11)),           # acqua
        (scroll_frames(no_border(tops[14])), load("lava_sides", 1), load("lava_sides", 2)),  # lava
        ([tops[10], tops[9], tops[10].rotate(180)], load("sides", 8), load("sides", 9)),  # boscaglia
    ]
    sheet = Image.new("RGBA", (5 * C, 7 * C), (0, 0, 0, 0))
    for r, (top3, side, side_top) in enumerate(rows):
        for c, im in enumerate(top3):
            top = opaque(no_border(im))
            # Acqua e lava scorrono: il filo si muoverebbe con loro.
            sheet.paste(top if r in (3, 4) else edge(top), (c * C, r * C))
        sheet.paste(opaque(fix_top_edge(side)), (3 * C, r * C))
        sheet.paste(opaque(fix_top_edge(side_top)), (4 * C, r * C))
    for c in range(4):
        sheet.alpha_composite(decor(c), (c * C, 6 * C))
    OUT.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(OUT)
    print(OUT.relative_to(OUT.parents[2]), sheet.size)


if __name__ == "__main__":
    main()

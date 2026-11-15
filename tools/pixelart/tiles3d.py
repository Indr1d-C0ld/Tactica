"""Texture della mappa 3D in stile VGA: facce quadrate 16x16 per le sommità
(3 varianti o fotogrammi d'animazione), fianchi 16x8 per livello e piccoli
sprite di decorazione (cespugli, sassi) da mostrare come cartelloni.

Atlante a celle di 16x16: righe = terreni nell'ordine di tiles.ORDER;
colonne 0-2 = sommità, colonna 3 = fianco (nelle prime 8 righe della cella).
Riga 6: decorazioni (cespuglio A, cespuglio B, sassi).
"""

from __future__ import annotations

from PIL import Image

from pixlib import Canvas, Mat, hash2, ramp
from tiles import ORDER, SIDES, TOPS

C = 16


def top(terrain: str, variant: int) -> Image.Image:
    img = Image.new("RGBA", (C, C))
    px = img.load()
    rp = TOPS[terrain]
    seed = ORDER.index(terrain) * 10 + variant
    for y in range(C):
        for x in range(C):
            n = hash2(x, y, seed)
            t = 2
            if terrain == "Grass":
                t = 1 if n < 0.2 else (3 if n > 0.88 else 2)
                if (x * 3 + y * 5 + variant) % 23 == 0:
                    t = 3
            elif terrain == "Thicket":
                t = 1 if n < 0.4 else (3 if n > 0.92 else 2)
            elif terrain == "Stone":
                gx = (x + (y // 6) * 4) % 8
                t = 1 if gx == 0 or y % 6 == 0 else (3 if n > 0.9 else 2)
            elif terrain == "Sand":
                t = 1 if (x + 2 * y + variant * 3) % 9 == 0 else (3 if n > 0.9 else 2)
            elif terrain == "Water":
                wave = (x + 2 * y + variant * 5) % 10
                t = 3 if wave == 0 else (1 if wave in (5, 6) and n < 0.5 else 2)
            elif terrain == "Lava":
                crust = hash2(x // 3, y // 3, ORDER.index(terrain)) < 0.45
                glow = (x + y + variant * 3) % 6 == 0
                t = 0 if crust and n < 0.6 else (1 if crust else (3 if glow else 2))
            # Bordo scuro sottile: la griglia resta leggibile da ogni angolazione.
            if x == 0 or y == 0 or x == C - 1 or y == C - 1:
                t = max(0, t - 1)
            px[x, y] = rp[t]
    if terrain == "Grass" and variant == 2:
        for fx, fy in ((4, 5), (11, 9), (7, 12)):
            px[fx, fy] = (250, 236, 200, 255)
            px[fx + 1, fy] = (240, 120, 160, 255)
    return img


def side(terrain: str) -> Image.Image:
    img = Image.new("RGBA", (C, C), (0, 0, 0, 0))
    px = img.load()
    rp = SIDES[terrain]
    seed = ORDER.index(terrain) + 40
    for y in range(8):
        for x in range(C):
            n = hash2(x, y, seed)
            t = 2
            if y == 0:
                t = 3 if terrain not in ("Water", "Lava") else 2
            elif y == 4 and n < 0.7:
                t = 1
            elif n > 0.93:
                t = 3
            elif n < 0.06:
                t = 1
            if y == 7:
                t = max(0, t - 1)
            px[x, y] = rp[t]
    return img


def bush(variant: int) -> Image.Image:
    c = Canvas(C, C)
    leaf = Mat(ramp("#143a1e", "#2a6a30", "#48a044", "#8cd068"))
    if variant == 0:
        c.ellipse(5.5, 10, 4.5, 4, leaf)
        c.ellipse(10.5, 9, 4.5, 4.5, leaf)
        c.ellipse(8, 6, 4, 3.5, leaf)
    else:
        c.ellipse(8, 9, 6, 5, leaf)
        c.ellipse(8, 5, 3.5, 3, leaf)
    berry = Mat(ramp("#8a1c2c", "#c42c3c", "#ee5a5a", "#ffa0a0"), flat=True, tone=2)
    for bx, by in ((6, 8), (10, 11), (9, 5)):
        if c.mat[by][bx] is leaf:
            c.put(bx, by, berry, tone=2)
    return c.render()


def pebbles() -> Image.Image:
    c = Canvas(C, C)
    m = Mat(ramp("#2c2e3a", "#585e70", "#8a90a2", "#c8cdd8"))
    c.ellipse(6, 12, 2.5, 2, m)
    c.ellipse(10.5, 13, 2, 1.6, m)
    return c.render()


def atlas() -> Image.Image:
    img = Image.new("RGBA", (C * 4, C * (len(ORDER) + 1)), (0, 0, 0, 0))
    for i, t in enumerate(ORDER):
        for v in range(3):
            img.paste(top(t, v), (v * C, i * C))
        img.paste(side(t), (3 * C, i * C))
    row = len(ORDER) * C
    img.paste(bush(0), (0, row))
    img.paste(bush(1), (C, row))
    img.paste(pebbles(), (2 * C, row))
    return img


if __name__ == "__main__":
    import sys

    a = atlas()
    a.resize((a.width * 8, a.height * 8), Image.NEAREST).save(sys.argv[1])

"""Caselle isometriche 2:1 (rombo 32x16) e fianchi da 8 pixel per livello.

Celle 32x32: per la sommità il rombo occupa le righe 16..31 (centro a (16, 24)),
lasciando 16 righe sopra per decorazioni in rilievo (cespugli, rocce).
Per i fianchi il rombo occupa le righe 0..15 (centro a (16, 8)) e le facce
scendono di 8 pixel.
"""

from __future__ import annotations

from PIL import Image

from pixlib import Canvas, Mat, darken, hash2, ramp

CELL = 32

TOPS = {
    "Grass": ramp("#1f4a2a", "#3a7a3a", "#5aa648", "#93d36a"),
    "Stone": ramp("#3a3d4c", "#646a7c", "#8e95a6", "#c3c9d4"),
    "Sand": ramp("#7a5a30", "#b8925a", "#dcbc7e", "#f4dfa8"),
    "Water": ramp("#173a6e", "#2a62a8", "#3f8ad0", "#9fd4f4"),
    "Lava": ramp("#4a1208", "#a82c10", "#ee6618", "#ffd860"),
    "Thicket": ramp("#173a22", "#2c6232", "#468a40", "#7cbc5c"),
}
SIDES = {
    "Grass": ramp("#3a2418", "#5e3c26", "#835636", "#a8744a"),
    "Thicket": ramp("#3a2418", "#5e3c26", "#835636", "#a8744a"),
    "Sand": ramp("#5c4024", "#8a6438", "#b08650", "#cfa76c"),
    "Stone": ramp("#2a2c38", "#474b5c", "#686e80", "#8a90a2"),
    "Water": ramp("#10264a", "#1c3c6c", "#2a5490", "#3f6eaa"),
    "Lava": ramp("#1e1416", "#36262a", "#4e3a3c", "#6a5254"),
}
ORDER = ["Grass", "Stone", "Sand", "Water", "Lava", "Thicket"]


def diamond_rows():
    """(riga, x0, x1) del rombo 32x16."""
    out = []
    for r in range(16):
        half = 2 * (r + 1) if r < 8 else 2 * (16 - r)
        out.append((r, 16 - half, 16 + half - 1))
    return out


DIAMOND = diamond_rows()


def in_diamond(x: int, r: int) -> bool:
    if not 0 <= r < 16:
        return False
    _, x0, x1 = DIAMOND[r]
    return x0 <= x <= x1


def top(terrain: str, variant: int) -> Image.Image:
    img = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
    px = img.load()
    rp = TOPS[terrain]
    seed = ORDER.index(terrain) * 10 + variant
    for r, x0, x1 in DIAMOND:
        for x in range(x0, x1 + 1):
            n = hash2(x, r, seed)
            t = 2
            if terrain == "Grass":
                t = 1 if n < 0.18 else (3 if n > 0.9 else 2)
            elif terrain == "Thicket":
                t = 1 if n < 0.35 else 2
            elif terrain == "Stone":
                # Lastre: fughe scure su una griglia sfalsata.
                gx = (x + (r // 5) * 3) % 9
                t = 1 if gx == 0 or r % 5 == 0 else (3 if n > 0.93 else 2)
            elif terrain == "Sand":
                t = 1 if (x + 2 * r + variant * 3) % 11 == 0 else (3 if n > 0.92 else 2)
            elif terrain == "Water":
                wave = (x + 2 * r + variant * 4) % 12
                t = 3 if wave == 0 else (1 if n < 0.1 else 2)
            elif terrain == "Lava":
                crust = hash2(x // 3, r // 2, seed // 10) < 0.45
                t = 0 if crust and n < 0.6 else (1 if crust else (3 if (x + r + variant * 2) % 7 == 0 else 2))
            # Smusso: bordi alti più chiari, bassi più scuri.
            if r < 8 and (x == x0 or x == x0 + 1 or x == x1 or x == x1 - 1):
                t = max(t, 3) if terrain not in ("Lava",) else t
            if r >= 8 and (x <= x0 + 1 or x >= x1 - 1):
                t = min(t, 1)
            px[x, r + 16] = rp[t]
    if terrain == "Grass" and variant == 2:
        for (fx, fr) in ((10, 6), (20, 9), (15, 11)):
            px[fx, fr + 16] = (250, 236, 200, 255)
            px[fx + 1, fr + 16] = (240, 120, 160, 255)
    if terrain == "Grass":
        for k in range(4):  # ciuffi d'erba in rilievo
            gx = 6 + int(hash2(k, variant, 77) * 20)
            gr = 3 + int(hash2(variant, k, 78) * 10)
            if in_diamond(gx, gr) and in_diamond(gx, gr - 2):
                px[gx, gr + 16] = rp[3]
                px[gx, gr + 15] = rp[3]
                px[gx + 1, gr + 16] = rp[2]
    if terrain == "Thicket":
        bushes(img, variant)
    if terrain == "Stone" and variant == 1:
        rock(img)
    return img


def bushes(img: Image.Image, variant: int):
    c = Canvas(CELL, CELL)
    leaf = Mat(ramp("#143a1e", "#2a6a30", "#48a044", "#8cd068"))
    spots = [(10, 22, 5.5, 4.5), (21, 21, 6, 5), (15, 26, 6, 4.5)]
    if variant == 1:
        spots = [(12, 20, 6, 5.5), (20, 25, 5.5, 4.5)]
    if variant == 2:
        spots = [(16, 21, 7, 6), (9, 26, 4.5, 3.5), (23, 26, 4.5, 3.5)]
    for cx, cy, rx, ry in spots:
        c.ellipse(cx, cy, rx, ry, leaf)
    for k in range(3):
        bx = int(8 + hash2(k, variant, 5) * 16)
        by = int(17 + hash2(variant, k, 6) * 8)
        if c.mat[by][bx] is leaf:
            c.put(bx, by, Mat(ramp("#8a1c2c", "#c42c3c", "#ee5a5a", "#ffa0a0"), flat=True, tone=2), tone=2)
    b = c.render()
    img.alpha_composite(b)


def rock(img: Image.Image):
    """Sassolini decorativi (piccoli, per non sembrare ostacoli)."""
    c = Canvas(CELL, CELL)
    m = Mat(ramp("#2c2e3a", "#585e70", "#8a90a2", "#c8cdd8"))
    c.ellipse(12, 25, 2, 1.5, m)
    c.ellipse(20, 22, 1.5, 1.2, m)
    img.alpha_composite(c.render())


def side(terrain: str) -> Image.Image:
    img = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
    px = img.load()
    rp = SIDES[terrain]
    seed = ORDER.index(terrain)
    for x in range(CELL):
        bottom = max(r for r in range(16) if in_diamond(x, r))
        for k in range(1, 9):
            y = bottom + k
            left = x < 16
            t = 2 if left else 1
            n = hash2(x, k, seed + 40)
            if k == 4 and n < 0.7:
                t -= 1  # stratificazione
            elif n > 0.94:
                t += 1
            if x in (15, 16):
                t = max(0, t - 1)  # spigolo
            px[x, y] = rp[max(0, min(3, t))]
        # La faccia superiore del segmento è coperta dai segmenti successivi:
        # la si riempie col colore del fianco per non lasciare buchi.
        for r in range(16):
            if in_diamond(x, r):
                px[x, r] = rp[1]
    return img


def overlays() -> list[Image.Image]:
    """Evidenziazione (bianca, da tingere), cursore, freccia, ombra."""
    hl = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
    p = hl.load()
    for r, x0, x1 in DIAMOND:
        for x in range(x0, x1 + 1):
            edge = x <= x0 + 1 or x >= x1 - 1
            if edge:
                p[x, r + 16] = (255, 255, 255, 230)
            elif (x + r) % 2 == 0:
                p[x, r + 16] = (255, 255, 255, 120)
            else:
                p[x, r + 16] = (255, 255, 255, 60)
    cur = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
    p = cur.load()
    for r, x0, x1 in DIAMOND:
        for x in (x0, x0 + 1, x1 - 1, x1):
            p[x, r + 16] = (255, 248, 210, 255)
    arrow = Canvas(CELL, CELL)
    gold = Mat(ramp("#6a3a10", "#c07a1c", "#f2c040", "#fff0a0"))
    arrow.poly([(11, 18), (21, 18), (16, 26)], gold)
    arrow.rect(14, 12, 18, 18, gold)
    arw = arrow.render()
    shadow = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
    p = shadow.load()
    for y in range(CELL):
        for x in range(CELL):
            dx = (x + 0.5 - 16) / 9
            dy = (y + 0.5 - 24) / 3.6
            if dx * dx + dy * dy <= 1:
                p[x, y] = (10, 8, 20, 110)
    return [hl, cur, arw, shadow]


def atlas() -> Image.Image:
    rows = len(ORDER) + 1
    img = Image.new("RGBA", (CELL * 4, CELL * rows), (0, 0, 0, 0))
    for i, t in enumerate(ORDER):
        for v in range(3):
            img.paste(top(t, v), (v * CELL, i * CELL))
        img.paste(side(t), (3 * CELL, i * CELL))
    for j, o in enumerate(overlays()):
        img.paste(o, (j * CELL, len(ORDER) * CELL))
    return img


if __name__ == "__main__":
    import sys

    a = atlas()
    a.resize((a.width * 5, a.height * 5), Image.NEAREST).save(sys.argv[1])
    _ = darken

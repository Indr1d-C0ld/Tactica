"""Texture della mappa 3D in stile pixel-art "naturale".

Atlante a celle 16x16. Righe = terreni (ordine di tiles.ORDER):
  colonne 0-2: sommità (varianti, o fotogrammi per acqua e lava);
  colonna 3: fianco (8 righe per livello);
  colonna 4: fianco del livello più alto, con il bordo della sommità che
  ricade (erba che scende, sabbia che si sbriciola...).
Riga 6: decorazioni (cespuglio A, cespuglio B, sassi, ciuffo d'erba).

Le texture usano gruppi di pixel invece del rumore pixel per pixel:
celle di Voronoi per lastre e placche, rumore a grana grossa per le macchie,
pochi dettagli disegnati (fili d'erba, fiori, spuma).
"""

from __future__ import annotations

from PIL import Image

from pixlib import Canvas, Mat, hash2, ramp
from tiles import ORDER, SIDES, TOPS

C = 16
COLS = 5


def coarse(x: int, y: int, seed: int, cell: int = 4) -> float:
    """Rumore a grana grossa (interpolazione bilineare su griglia)."""
    gx, gy = x // cell, y // cell
    fx, fy = (x % cell) / cell, (y % cell) / cell
    a = hash2(gx % 4, gy % 4, seed)
    b = hash2((gx + 1) % 4, gy % 4, seed)
    c = hash2(gx % 4, (gy + 1) % 4, seed)
    d = hash2((gx + 1) % 4, (gy + 1) % 4, seed)
    return (a * (1 - fx) + b * fx) * (1 - fy) + (c * (1 - fx) + d * fx) * fy


def voronoi(seed: int, n: int):
    """Punti di Voronoi ripetibili (la texture si affianca senza giunte)."""
    pts = [(hash2(i, 1, seed) * C, hash2(i, 2, seed) * C) for i in range(n)]

    def nearest(x: float, y: float):
        best = (1e9, 1e9, -1)
        for i, (px, py) in enumerate(pts):
            for ox in (-C, 0, C):
                for oy in (-C, 0, C):
                    d = (x - px - ox) ** 2 + (y - py - oy) ** 2
                    if d < best[0]:
                        best = (d, best[0], i)
                    elif d < best[1]:
                        best = (best[0], d, best[2])
        return best[2], (best[1] ** 0.5 - best[0] ** 0.5)

    return pts, nearest


def top(terrain: str, variant: int) -> Image.Image:
    img = Image.new("RGBA", (C, C))
    px = img.load()
    rp = TOPS[terrain]
    seed = ORDER.index(terrain) * 10 + variant
    if terrain in ("Stone", "Lava"):
        pts, nearest = voronoi(seed + (0 if terrain == "Stone" else 50), 6 if terrain == "Stone" else 5)
    for y in range(C):
        for x in range(C):
            n = hash2(x, y, seed)
            t = 2
            if terrain in ("Grass", "Thicket"):
                m = coarse(x, y, seed, 4)
                t = 1 if m < 0.33 else (2 if m < 0.72 else 3)
                if terrain == "Thicket":
                    t = max(0, t - 1)
                if n > 0.94:
                    t = min(3, t + 1)
            elif terrain == "Stone":
                i, edge = nearest(x + 0.5, y + 0.5)
                if edge < 1.0:
                    t = 0  # fuga fra le lastre
                else:
                    shade = hash2(i, 9, seed)
                    t = 2 if shade < 0.6 else 3
                    # Bordo chiaro in alto a sinistra di ogni lastra, scuro in basso a destra.
                    j1, e1 = nearest(x - 0.5, y - 0.5)
                    if j1 != i:
                        t = 3
                    j2, e2 = nearest(x + 1.5, y + 1.5)
                    if j2 != i:
                        t = 1
            elif terrain == "Sand":
                m = coarse(x, y, seed, 4)
                ripple = (x + y * 2 + int(m * 6) + variant * 3) % 8
                t = 1 if ripple == 0 else (3 if ripple == 1 else 2)
                if n > 0.95:
                    t = 1
            elif terrain == "Water":
                m = coarse(x, y, seed, 8)
                band = (y * 2 + x + variant * 4 + int(m * 5)) % 11
                t = 3 if band == 0 else (2 if band < 6 else 1)
                if band == 1 and n < 0.4:
                    t = 3
            elif terrain == "Lava":
                i, edge = nearest(x + 0.5, y + 0.5)
                glow = (variant + i) % 3
                if edge < 1.2:
                    t = 3 if (x + y + variant) % 3 else 2  # crepe incandescenti
                else:
                    t = 0 if hash2(i, 7, seed) < 0.5 else 1
                    if edge < 2.2 and glow == 0:
                        t = 1
            px[x, y] = rp[t]
    # Dettagli disegnati.
    if terrain == "Grass":
        for k in range(3 + variant):
            gx = int(hash2(k, variant, 31) * 14) + 1
            gy = int(hash2(variant, k, 32) * 13) + 2
            px[gx, gy] = rp[3]
            px[gx, gy - 1] = rp[3]
            px[gx + 1, gy] = rp[2]
        if variant == 2:
            for fx, fy, col in ((4, 5, (250, 236, 200, 255)), (11, 10, (240, 140, 170, 255)), (7, 13, (250, 220, 120, 255))):
                px[fx, fy] = col
    return img


def side(terrain: str, lip: bool) -> Image.Image:
    img = Image.new("RGBA", (C, C), (0, 0, 0, 0))
    px = img.load()
    rp = SIDES[terrain]
    seed = ORDER.index(terrain) + 40
    pts, nearest = voronoi(seed, 4)
    for y in range(8):
        for x in range(C):
            m = coarse(x, y * 2, seed, 4)
            t = 2 if m > 0.35 else 1
            if terrain in ("Grass", "Thicket", "Sand") and hash2(x // 3, y // 2, seed) > 0.86:
                t = 3  # sassi annegati nella terra
            if y == 7:
                t = max(0, t - 1)
            if terrain == "Stone":
                i, edge = nearest(x + 0.5, y * 2 + 0.5)
                t = 0 if edge < 1.0 else (2 if hash2(i, 3, seed) < 0.6 else 1)
            px[x, y] = rp[t]
    if lip:
        top_rp = TOPS[terrain]
        if terrain in ("Grass", "Thicket"):
            # L'erba ricade sul fianco con frange irregolari.
            for x in range(C):
                depth = 1 + int(hash2(x, 3, seed) * 3) + (1 if x % 5 == 2 else 0)
                for y in range(min(depth, 5)):
                    px[x, y] = top_rp[2 if y == 0 else 1]
                if depth < 5:
                    px[x, depth] = top_rp[0]
        elif terrain == "Sand":
            for x in range(C):
                depth = 1 + int(hash2(x, 5, seed) * 2)
                for y in range(depth):
                    px[x, y] = top_rp[2 if y == 0 else 1]
        elif terrain == "Stone":
            for x in range(C):
                px[x, 0] = top_rp[3]
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


def tuft() -> Image.Image:
    img = Image.new("RGBA", (C, C), (0, 0, 0, 0))
    p = img.load()
    rp = TOPS["Grass"]
    for x, h in ((5, 4), (6, 6), (7, 3), (9, 5), (10, 7), (11, 4)):
        for y in range(h):
            p[x, 15 - y] = rp[3 if y == h - 1 else 2]
        p[x, 15] = rp[1]
    return img


def atlas() -> Image.Image:
    img = Image.new("RGBA", (C * COLS, C * (len(ORDER) + 1)), (0, 0, 0, 0))
    for i, t in enumerate(ORDER):
        for v in range(3):
            img.paste(top(t, v), (v * C, i * C))
        img.paste(side(t, False), (3 * C, i * C))
        img.paste(side(t, True), (4 * C, i * C))
    row = len(ORDER) * C
    img.paste(bush(0), (0, row))
    img.paste(bush(1), (C, row))
    img.paste(pebbles(), (2 * C, row))
    img.paste(tuft(), (3 * C, row))
    return img


def backdrop(w: int = 480, h: int = 270) -> Image.Image:
    """Fondale: cielo a bande con retino, nuvole a pixel, colline lontane."""
    img = Image.new("RGBA", (w, h))
    p = img.load()
    bands = [(28, 36, 74), (40, 54, 104), (58, 80, 138), (86, 112, 166), (122, 146, 186), (164, 178, 204)]
    for y in range(h):
        t = y / (h * 0.75)
        f = min(len(bands) - 1.0, t * (len(bands) - 1))
        i = int(f)
        frac = f - i
        for x in range(w):
            # Retino ordinato fra una banda e la successiva.
            bayer = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]][y % 4][x % 4] / 16
            c = bands[min(i + 1, len(bands) - 1)] if frac > bayer else bands[i]
            p[x, y] = c + (255,)
    # Nuvole a cumulo: più cerchi, base piatta, ombra in basso, luce in alto.
    import math
    light, mid, dark = (232, 236, 246, 255), (200, 210, 232, 255), (156, 170, 206, 255)
    for k in range(6):
        cx = int(hash2(k, 1, 77) * w)
        base_y = int(40 + hash2(k, 2, 77) * 80)
        n = 4 + int(hash2(k, 3, 77) * 3)
        blobs = []
        for j in range(n):
            r = 5 + int(hash2(k, j + 10, 77) * 7)
            bx = cx + j * 9 - n * 4
            blobs.append((bx, base_y - r // 2, r))
        for y in range(base_y - 20, base_y + 1):
            for x in range(cx - n * 6 - 12, cx + n * 6 + 12):
                if not (0 <= x < w and 0 <= y < h):
                    continue
                inside = [(bx, by, r) for bx, by, r in blobs if (x - bx) ** 2 + (y - by) ** 2 <= r * r]
                if not inside:
                    continue
                bx, by, r = inside[0]
                c = mid
                if y >= base_y - 2:
                    c = dark
                elif (y - by) < -r * 0.45:
                    c = light
                p[x, y] = c
    # Colline lontane in due strati, profilo morbido.
    for layer, (base, amp, col) in enumerate([(h - 82, 26, (66, 86, 126)), (h - 52, 20, (46, 62, 96))]):
        for x in range(w):
            prof = 0.5 + 0.3 * math.sin(x / (41 - layer * 9) + layer * 2.1) + 0.2 * math.sin(x / 13.0 + layer)
            hy = base - int(amp * prof) - int(3 * hash2(x // 6, layer, 91))
            for y in range(max(0, hy), h):
                p[x, y] = col + (255,)
            if 0 <= hy < h:
                p[x, hy] = tuple(min(255, v + 18) for v in col) + (255,)
    return img


if __name__ == "__main__":
    import sys

    a = atlas()
    a.resize((a.width * 8, a.height * 8), Image.NEAREST).save(sys.argv[1])
    if len(sys.argv) > 2:
        backdrop().resize((960, 540), Image.NEAREST).save(sys.argv[2])


def worldmap(w: int = 480, h: int = 270) -> Image.Image:
    """Mappa del mondo su pergamena: cinque regioni da ovest a est, con
    paesaggi a pixel (alberi, canneti, dune, vulcano, montagne) e mare ai bordi."""
    import math

    img = Image.new("RGBA", (w, h))
    p = img.load()
    parch = [(232, 214, 170), (220, 199, 152), (204, 180, 132), (176, 150, 104)]
    tints = [(120, 168, 90), (96, 150, 140), (214, 176, 100), (170, 96, 70), (140, 150, 170)]
    for y in range(h):
        for x in range(w):
            n = coarse(x // 2 % C, y // 2 % C, 300, 4)
            t = 1 if n < 0.35 else 0
            if hash2(x, y, 301) > 0.97:
                t = 2
            c = parch[t]
            # Colore della regione, sfumato a retino.
            r = min(4, int(x / w * 5))
            fx = (x / w * 5) % 1
            bayer = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]][y % 4][x % 4] / 16
            if bayer < 0.35 or (fx > 0.85 and bayer < 0.15):
                tint = tints[r]
                c = tuple(int(c[k] * 0.55 + tint[k] * 0.45) for k in range(3))
            # Mare e bordo.
            coast = 10 + 6 * math.sin(x / 23.0) + 4 * math.sin(x / 7.0 + 1)
            if y < coast * 0.6 or y > h - coast or x < 6 + 3 * math.sin(y / 9.0):
                c = (84, 128, 168) if (x + y) % 3 else (110, 150, 186)
            p[x, y] = c + (255,)
    # Bordo scuro della pergamena.
    for y in range(h):
        for x in range(w):
            d = min(x, y, w - 1 - x, h - 1 - y)
            if d < 3:
                p[x, y] = (110, 82, 52, 255) if d < 2 else (150, 116, 76, 255)

    def tree(cx, cy):
        for dy in range(5):
            for dx in range(-2 + dy // 2, 3 - dy // 2):
                p[cx + dx, cy - dy] = (52, 96, 52, 255) if dx < 1 else (36, 70, 40, 255)
        p[cx, cy + 1] = (90, 60, 40, 255)

    def reed(cx, cy):
        for dy in range(4):
            p[cx, cy - dy] = (60, 110, 96, 255)
        p[cx + 1, cy - 3] = (140, 110, 60, 255)

    def dune(cx, cy):
        for dx in range(-5, 6):
            hgt = int(3 * math.cos(dx / 5 * math.pi / 2))
            for dy in range(hgt):
                p[cx + dx, cy - dy] = (196, 150, 80, 255) if dx < 0 else (170, 126, 64, 255)

    def mountain(cx, cy, size, snow=True):
        for dy in range(size):
            for dx in range(-dy, dy + 1):
                col = (110, 110, 128, 255) if dx < 0 else (80, 80, 100, 255)
                if snow and dy < size // 3:
                    col = (236, 236, 244, 255)
                p[cx + dx, cy - size + dy] = col

    def volcano(cx, cy):
        mountain(cx, cy, 14, snow=False)
        for dx in range(-2, 3):
            p[cx + dx, cy - 14] = (230, 90, 30, 255)
        for k in range(4):
            p[cx + k - 1, cy - 16 - k] = (90, 80, 80, 255)

    for k in range(14):
        tree(int(20 + hash2(k, 1, 7) * 70), int(80 + hash2(k, 2, 7) * 150))
    for k in range(16):
        reed(int(105 + hash2(k, 1, 8) * 80), int(70 + hash2(k, 2, 8) * 160))
    for k in range(8):
        dune(int(205 + hash2(k, 1, 9) * 70), int(80 + hash2(k, 2, 9) * 150))
    volcano(330, 120)
    volcano(355, 200)
    for k in range(6):
        mountain(int(400 + hash2(k, 1, 10) * 60), int(90 + hash2(k, 2, 10) * 140), 12 + k % 3 * 3)
    return img

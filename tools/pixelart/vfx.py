"""Effetti visivi delle abilità: strisce di fotogrammi 32x32 (atlante vfx.png).

Righe (nell'ordine di EFFECTS), 6 fotogrammi per riga; i fotogrammi oltre
la durata dell'effetto restano vuoti. La riga "arrow" contiene il proiettile
nel primo fotogramma.
"""

from __future__ import annotations

import math

from PIL import Image

from pixlib import hash2

S = 32
FRAMES = 6
EFFECTS = ["slash", "arrow", "impact", "fire", "ice", "thunder", "holy", "heal", "buff", "debuff", "poison", "drain"]
LENGTHS = {"slash": 4, "arrow": 1, "impact": 4, "fire": 6, "ice": 5, "thunder": 4, "holy": 5, "heal": 6,
           "buff": 5, "debuff": 5, "poison": 6, "drain": 5}


def rgba(h: str, a: int = 255):
    h = h.lstrip("#")
    return (int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16), a)


def put(img, x, y, c):
    if 0 <= x < S and 0 <= y < S:
        img.putpixel((int(x), int(y)), c)


def disc(img, cx, cy, r, c):
    for y in range(int(cy - r), int(cy + r) + 1):
        for x in range(int(cx - r), int(cx + r) + 1):
            if (x + 0.5 - cx) ** 2 + (y + 0.5 - cy) ** 2 <= r * r:
                put(img, x, y, c)


def slash(f: int) -> Image.Image:
    img = Image.new("RGBA", (S, S))
    # Arco che si apre da sinistra-alto verso destra-basso.
    a0 = -2.4 + f * 0.55
    for k in range(14):
        a = a0 + k * 0.09
        for r in range(9, 14):
            x = 16 + math.cos(a) * r
            y = 16 + math.sin(a) * r
            col = rgba("#ffffff") if r in (11, 12) else rgba("#bfe0ff")
            if k < 3:
                col = rgba("#7aa8e0")
            put(img, x, y, col)
    return img


def arrow() -> Image.Image:
    img = Image.new("RGBA", (S, S))
    for x in range(8, 24):
        put(img, x, 16, rgba("#8a6a3a"))
    for d in range(3):
        put(img, 24 + d, 16, rgba("#d8dde8"))
        put(img, 24 - d, 15 - d, rgba("#d8dde8")) if d == 0 else None
    put(img, 26, 15, rgba("#d8dde8"))
    put(img, 26, 17, rgba("#d8dde8"))
    for d in range(3):
        put(img, 8 + d, 15 - d // 2, rgba("#e8e0c8"))
        put(img, 8 + d, 17 + d // 2, rgba("#e8e0c8"))
    return img


def impact(f: int) -> Image.Image:
    img = Image.new("RGBA", (S, S))
    r = 3 + f * 3
    for k in range(8):
        a = k * math.pi / 4 + 0.3
        for t in range(2):
            put(img, 16 + math.cos(a) * (r + t), 16 + math.sin(a) * (r + t), rgba("#fff6c8" if f < 2 else "#ffc860"))
    if f < 2:
        disc(img, 16, 16, 3 - f, rgba("#ffffff"))
    return img


def fire(f: int) -> Image.Image:
    img = Image.new("RGBA", (S, S))
    cols = [rgba("#fff3a0"), rgba("#ffc23a"), rgba("#f2661c"), rgba("#a82810")]
    for k in range(10):
        bx = 6 + hash2(k, 1, 3) * 20
        h = (6 + hash2(k, 2, 3) * 12) * (0.4 + 0.6 * math.sin(min(math.pi, (f + 1) / LENGTHS["fire"] * math.pi)))
        for y in range(int(h)):
            w = max(1, int((1 - y / max(h, 1)) * 3))
            for x in range(-w, w + 1):
                c = cols[min(3, int(y / max(h, 1) * 4))] if abs(x) < w else cols[3]
                put(img, bx + x + math.sin(y * 0.6 + f) * 1.2, 28 - y, c)
    return img


def ice(f: int) -> Image.Image:
    img = Image.new("RGBA", (S, S))
    grow = min(1.0, (f + 1) / 3)
    fade = f >= 4
    for k in range(6):
        bx = 6 + k * 4 + int(hash2(k, 1, 5) * 3)
        h = int((6 + hash2(k, 2, 5) * 12) * grow)
        for y in range(h):
            w = max(0, int((1 - y / max(h, 1)) * 2))
            for x in range(-w, w + 1):
                c = rgba("#e8fbff") if x == -w else (rgba("#7ccaff") if x < w else rgba("#3b7ce2"))
                if fade and (x + y) % 2:
                    continue
                put(img, bx + x, 28 - y, c)
    return img


def thunder(f: int) -> Image.Image:
    img = Image.new("RGBA", (S, S))
    if f == 3:
        disc(img, 16, 26, 5, rgba("#fff6a0", 200))
        return img
    x = 16
    for y in range(0, 28):
        x += int((hash2(y, f, 9) - 0.5) * 4)
        x = max(8, min(24, x))
        put(img, x, y, rgba("#ffffff"))
        put(img, x + 1, y, rgba("#fff07a"))
        put(img, x - 1, y, rgba("#c8a8ff"))
    disc(img, x, 27, 2 + f, rgba("#fff07a"))
    return img


def holy(f: int) -> Image.Image:
    img = Image.new("RGBA", (S, S))
    h = 8 + f * 5
    for y in range(28 - h, 29):
        for x in range(13, 20):
            edge = x in (13, 19)
            put(img, x, y, rgba("#fff8d8", 160 if edge else 230))
    for k in range(4):
        a = f * 0.8 + k * math.pi / 2
        put(img, 16 + math.cos(a) * 9, 18 + math.sin(a) * 4, rgba("#ffe48a"))
    return img


def heal(f: int) -> Image.Image:
    img = Image.new("RGBA", (S, S))
    for k in range(7):
        x = 6 + hash2(k, 1, 7) * 20
        y = 28 - ((hash2(k, 2, 7) * 10 + f * 4) % 26)
        c = rgba("#c8ffb0") if k % 2 else rgba("#5ae07a")
        put(img, x, y, c)
        put(img, x - 1, y, c)
        put(img, x + 1, y, c)
        put(img, x, y - 1, c)
        put(img, x, y + 1, c)
    return img


def ring(f: int, light: str, dark: str, down: bool) -> Image.Image:
    img = Image.new("RGBA", (S, S))
    y0 = 6 + f * 4 if down else 26 - f * 4
    for k in range(16):
        a = k * math.pi / 8
        put(img, 16 + math.cos(a) * 9, y0 + math.sin(a) * 3, rgba(light if k % 2 else dark))
    for k in range(3):
        put(img, 10 + k * 6, y0 - 4 + (f + k) % 3 * 2, rgba(light))
    return img


def poison(f: int) -> Image.Image:
    img = Image.new("RGBA", (S, S))
    for k in range(5):
        x = 8 + hash2(k, 1, 11) * 16
        y = 26 - ((hash2(k, 2, 11) * 8 + f * 3) % 20)
        r = 1 + (k + f) % 2
        disc(img, x, y, r + 0.5, rgba("#6ad04a"))
        put(img, x - r / 2, y - r / 2, rgba("#c8ff9a"))
    return img


def drain(f: int) -> Image.Image:
    img = Image.new("RGBA", (S, S))
    for k in range(6):
        a = k * math.pi / 3 + f * 0.6
        r = 12 - f * 2
        put(img, 16 + math.cos(a) * r, 16 + math.sin(a) * r * 0.6, rgba("#ff6a7a"))
        put(img, 16 + math.cos(a) * (r - 1), 16 + math.sin(a) * (r - 1) * 0.6, rgba("#a8203a"))
    return img


def atlas() -> Image.Image:
    img = Image.new("RGBA", (S * FRAMES, S * len(EFFECTS)), (0, 0, 0, 0))
    gens = {
        "slash": slash, "impact": impact, "fire": fire, "ice": ice, "thunder": thunder, "holy": holy,
        "heal": heal, "poison": poison, "drain": drain,
        "buff": lambda f: ring(f, "#ffe48a", "#fff8d8", False),
        "debuff": lambda f: ring(f, "#c88aff", "#6a3aa8", True),
    }
    for row, name in enumerate(EFFECTS):
        if name == "arrow":
            img.paste(arrow(), (0, row * S))
            continue
        for f in range(LENGTHS[name]):
            img.paste(gens[name](f), (f * S, row * S))
    return img


if __name__ == "__main__":
    import sys

    a = atlas()
    bg = Image.new("RGBA", a.size, (30, 34, 50, 255))
    bg.alpha_composite(a)
    bg.resize((a.width * 4, a.height * 4), Image.NEAREST).save(sys.argv[1])

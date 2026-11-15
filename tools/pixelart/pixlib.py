"""Mini-libreria per pixel-art procedurale.

Si disegnano forme (ellissi, rettangoli, poligoni, timbri ASCII) su una tela
a livelli: ogni operazione ha un materiale (rampa di 4 colori, dal più scuro
al più chiaro) e un id di "parte" crescente (profondità). Alla resa:

- ombreggiatura a smusso con luce dall'alto a sinistra (chiaro sui bordi
  alti/sinistri, scuro su quelli bassi/destri);
- linee interne scure dove una parte copre una parte più arretrata;
- contorno esterno colorato ("selout"): il colore più scuro del materiale
  vicino, ulteriormente scurito.
"""

from __future__ import annotations

from dataclasses import dataclass

from PIL import Image


def hexc(h: str) -> tuple[int, int, int, int]:
    h = h.lstrip("#")
    return (int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16), 255)


def ramp(*hexes: str) -> list[tuple[int, int, int, int]]:
    assert len(hexes) == 4
    return [hexc(h) for h in hexes]


def darken(c, f: float):
    return (int(c[0] * f), int(c[1] * f), int(c[2] * f), c[3])


@dataclass
class Mat:
    """Materiale: rampa di 4 toni; `flat` disattiva l'ombreggiatura (es. occhi)."""

    ramp: list
    flat: bool = False
    tone: int = 2  # tono base (0..3) per i materiali piatti
    outline: bool = True


class Canvas:
    def __init__(self, w: int, h: int):
        self.w, self.h = w, h
        self.mat: list[list[Mat | None]] = [[None] * w for _ in range(h)]
        self.part = [[-1] * w for _ in range(h)]
        self.tone_override: dict[tuple[int, int], int] = {}
        self._next = 0

    # ------------------------------------------------------------ primitive

    def new_part(self) -> int:
        self._next += 1
        return self._next

    def put(self, x: int, y: int, m: Mat, part: int | None = None, tone: int | None = None):
        if 0 <= x < self.w and 0 <= y < self.h:
            self.mat[y][x] = m
            self.part[y][x] = self._next if part is None else part
            if tone is None:
                self.tone_override.pop((x, y), None)
            else:
                self.tone_override[(x, y)] = tone

    def erase(self, x: int, y: int):
        if 0 <= x < self.w and 0 <= y < self.h:
            self.mat[y][x] = None
            self.part[y][x] = -1
            self.tone_override.pop((x, y), None)

    def rect(self, x0, y0, x1, y1, m: Mat, tone=None):
        p = self.new_part()
        for y in range(y0, y1 + 1):
            for x in range(x0, x1 + 1):
                self.put(x, y, m, p, tone)
        return p

    def ellipse(self, cx: float, cy: float, rx: float, ry: float, m: Mat, clip=None, tone=None):
        p = self.new_part()
        for y in range(self.h):
            for x in range(self.w):
                dx = (x + 0.5 - cx) / rx
                dy = (y + 0.5 - cy) / ry
                if dx * dx + dy * dy <= 1.0 and (clip is None or clip(x, y)):
                    self.put(x, y, m, p, tone)
        return p

    def poly(self, pts, m: Mat, tone=None):
        """Riempie un poligono (coordinate dei centri dei pixel)."""
        p = self.new_part()
        for y in range(self.h):
            for x in range(self.w):
                if _inside(x + 0.5, y + 0.5, pts):
                    self.put(x, y, m, p, tone)
        return p

    def line(self, x0, y0, x1, y1, m: Mat, tone=None, new_part=True):
        p = self.new_part() if new_part else self._next
        dx, dy = abs(x1 - x0), -abs(y1 - y0)
        sx, sy = (1 if x0 < x1 else -1), (1 if y0 < y1 else -1)
        err = dx + dy
        while True:
            self.put(x0, y0, m, p, tone)
            if x0 == x1 and y0 == y1:
                break
            e2 = 2 * err
            if e2 >= dy:
                err += dy
                x0 += sx
            if e2 <= dx:
                err += dx
                y0 += sy
        return p

    def stamp(self, art: str, x0: int, y0: int, mapping: dict, same_part=False):
        """Timbro ASCII: ogni carattere -> (Mat, tono) oppure Mat; '.' trasparente."""
        p = self._next if same_part else self.new_part()
        for j, row in enumerate(art.strip("\n").split("\n")):
            for i, ch in enumerate(row):
                if ch == "." or ch == " ":
                    continue
                v = mapping[ch]
                if isinstance(v, tuple):
                    self.put(x0 + i, y0 + j, v[0], p, v[1])
                else:
                    self.put(x0 + i, y0 + j, v, p)
        return p

    # ------------------------------------------------------------ resa

    def render(self, outline_color=None, inner_lines=True) -> Image.Image:
        img = Image.new("RGBA", (self.w, self.h), (0, 0, 0, 0))
        px = img.load()
        get = lambda x, y: self.mat[y][x] if 0 <= x < self.w and 0 <= y < self.h else None
        part = lambda x, y: self.part[y][x] if 0 <= x < self.w and 0 <= y < self.h else -1
        # Riquadro di ogni parte, per un'ombreggiatura "di volume".
        bbox: dict[int, list[int]] = {}
        for y in range(self.h):
            for x in range(self.w):
                if self.mat[y][x] is not None:
                    b = bbox.setdefault(self.part[y][x], [x, y, x, y])
                    b[0], b[1] = min(b[0], x), min(b[1], y)
                    b[2], b[3] = max(b[2], x), max(b[3], y)
        for y in range(self.h):
            for x in range(self.w):
                m = self.mat[y][x]
                if m is None:
                    continue
                me = self.part[y][x]
                if (x, y) in self.tone_override:
                    t = self.tone_override[(x, y)]
                elif m.flat:
                    t = m.tone
                else:
                    up, left = get(x, y - 1), get(x - 1, y)
                    down, right = get(x, y + 1), get(x + 1, y)
                    lit_edge = up is None or left is None or (up is not m and part(x, y - 1) < me)
                    dark_edge = down is None or right is None or (down is not m and part(x, y + 1) < me)
                    x0, y0, x1, y1 = bbox[me]
                    bw, bh = x1 - x0, y1 - y0
                    if bw >= 3 and bh >= 3:
                        # Luce dall'alto a sinistra sul volume della parte, con
                        # passaggi fra i toni a retino a scacchiera.
                        sv = (x - x0) / bw * 0.45 + (y - y0) / bh * 0.55
                        t = 3 if sv < 0.26 else (2 if sv < 0.7 else 1)
                        if 0.22 <= sv < 0.3:
                            t = 3 if (x + y) % 2 == 0 else 2
                        elif 0.66 <= sv < 0.74:
                            t = 2 if (x + y) % 2 == 0 else 1
                        if dark_edge:
                            t = min(t, 1)
                        elif lit_edge and t >= 2:
                            t = 3
                    else:
                        t = 2
                        if lit_edge:
                            t = 3
                        if dark_edge:
                            t = 1
                if inner_lines and not m.flat:
                    # Linea interna dove questa parte copre una parte più arretrata.
                    for nx, ny in ((x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)):
                        n = get(nx, ny)
                        if n is not None and n is not m and part(nx, ny) < me and n.outline:
                            t = 0
                            break
                px[x, y] = m.ramp[t]
        # Contorno esterno.
        out = img.copy()
        opx = out.load()
        for y in range(self.h):
            for x in range(self.w):
                if self.mat[y][x] is not None:
                    continue
                for nx, ny in ((x + 1, y), (x, y + 1), (x - 1, y), (x, y - 1)):
                    n = get(nx, ny)
                    if n is not None and n.outline:
                        # Contorno più morbido sul lato illuminato (alto/sinistra).
                        lit = nx > x or ny > y
                        opx[x, y] = outline_color or darken(n.ramp[0], 0.78 if lit else 0.5)
                        break
        return out


def _inside(x: float, y: float, pts) -> bool:
    inside = False
    n = len(pts)
    j = n - 1
    for i in range(n):
        xi, yi = pts[i]
        xj, yj = pts[j]
        if (yi > y) != (yj > y) and x < (xj - xi) * (y - yi) / (yj - yi + 1e-9) + xi:
            inside = not inside
        j = i
    return inside


def hash2(x: int, y: int, seed: int = 0) -> float:
    """Rumore deterministico in [0, 1)."""
    h = (x * 374761393 + y * 668265263 + seed * 2147483647) & 0xFFFFFFFF
    h = (h ^ (h >> 13)) * 1274126177 & 0xFFFFFFFF
    return ((h ^ (h >> 16)) & 0xFFFF) / 65536.0

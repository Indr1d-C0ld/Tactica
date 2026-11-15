"""Sprite chibi delle classi (design originali).

Tela 32x32 per fotogramma; il personaggio occupa circa 24x28 pixel con i piedi
a y=30 e il centro a x=16. Vista "fronte" = rivolto in basso a destra
(i fotogrammi si specchiano per il basso a sinistra); vista "retro" = rivolto
in alto a destra (specchiata per l'alto a sinistra).
"""

from __future__ import annotations

from dataclasses import dataclass

from PIL import Image

import palette as P
from pixlib import Canvas, Mat

W, H = 32, 32
OX = 4  # scostamento orizzontale del disegno (progettato su 24 colonne)

FRAMES = [
    "front_idle0", "front_idle1", "back_idle0", "back_idle1", "front_act", "back_act", "ko",
    "front_walk0", "front_walk1", "back_walk0", "back_walk1",
]

# Passo della camminata corrente: 0 fermo, +1/-1 gamba vicina avanti/indietro.
STEP = 0


@dataclass
class Ctx:
    team: Mat
    team_dark: Mat
    hair: Mat
    skin: Mat
    b: int  # oscillazione (0/1): sposta testa e busto
    back: bool
    act: bool


class C(Canvas):
    """Tela con scostamento orizzontale."""

    def __init__(self):
        super().__init__(W, H)

    def r(self, x0, y0, x1, y1, m, tone=None):
        return self.rect(x0 + OX, y0, x1 + OX, y1, m, tone)

    def e(self, cx, cy, rx, ry, m, clip=None, tone=None):
        cl = None if clip is None else (lambda x, y: clip(x - OX, y))
        return self.ellipse(cx + OX, cy, rx, ry, m, cl, tone)

    def p(self, pts, m, tone=None):
        return self.poly([(x + OX, y) for x, y in pts], m, tone)

    def px(self, x, y, m, tone=None, part=None):
        self.put(x + OX, y, m, part if part is not None else self._next, tone)

    def ln(self, x0, y0, x1, y1, m, tone=None):
        return self.line(x0 + OX, y0, x1 + OX, y1, m, tone)


M = lambda r: Mat(r)  # noqa: E731
METAL, GOLD, LEATHER, WOOD = M(P.METAL), M(P.GOLD), M(P.LEATHER), M(P.WOOD)
CREAM, GREEN, PURPLE, DARK = M(P.CREAM), M(P.GREEN), M(P.PURPLE), M(P.DARK)
GLOW, ROSE, WHITE = M(P.GLOW_BLUE), M(P.GLOW_ROSE), M(P.WHITE)


# ------------------------------------------------------------------ parti comuni

def legs(c: C, pants: Mat, boots: Mat):
    # Nel passo una gamba avanza (verso destra) e l'altra arretra sollevandosi.
    far = -STEP
    near = STEP
    c.r(9 + far, 25, 11 + far, 28 - (1 if far < 0 else 0), pants)
    c.r(9 + far, 28 - (1 if far < 0 else 0), 12 + far, 30 - (1 if far < 0 else 0), boots)
    c.r(13 + near, 25, 15 + near, 28 - (1 if near < 0 else 0), pants)
    c.r(13 + near, 28 - (1 if near < 0 else 0), 16 + near, 30 - (1 if near < 0 else 0), boots)


def far_arm(x: Ctx, c: C, sleeve: Mat):
    b = x.b
    c.r(6, 19 + b, 7, 23 + b, sleeve)
    c.r(6, 24 + b, 7, 24 + b, x.skin)


def near_arm(x: Ctx, c: C, sleeve: Mat):
    b = x.b
    if x.act:
        c.r(16, 16 + b, 17, 21 + b, sleeve)
        c.r(17, 15 + b, 18, 15 + b, x.skin)
    else:
        c.r(16, 19 + b, 17, 23 + b, sleeve)
        c.r(16, 24 + b, 17, 24 + b, x.skin)


def hand_pos(x: Ctx) -> tuple[int, int]:
    """Posizione (in coordinate di progetto) della mano che impugna l'arma."""
    return (18, 15 + x.b) if x.act else (17, 24 + x.b)


def torso(x: Ctx, c: C, cloth: Mat, belt: Mat | None = LEATHER, long_robe=False):
    b = x.b
    if long_robe:
        c.p([(8, 18 + b), (16, 18 + b), (18, 30.5), (6, 30.5)], cloth)
    else:
        c.p([(8, 18 + b), (16, 18 + b), (17, 26.5 + b), (7, 26.5 + b)], cloth)
    if belt is not None:
        c.r(8, 23 + b, 16, 23 + b, belt)


def head(x: Ctx, c: C, show_hair=True, bangs=True):
    b = x.b
    c.e(12.5, 11.5 + b, 6.5, 6.2, x.skin)
    if x.back:
        if show_hair:
            c.e(12.5, 11.2 + b, 6.9, 6.5, x.hair, clip=lambda px, py: py <= 16 + b)
            c.r(15, 15 + b, 16, 16 + b, x.skin, tone=1)  # nuca
        return
    if show_hair:
        def hair_clip(px, py):
            return py <= 9 + b or px <= 8 or (px <= 10 and py <= 12 + b)
        c.e(12.5, 11.2 + b, 6.9, 6.5, x.hair, clip=hair_clip)
        if bangs:
            for bx in (11, 13, 15):
                c.px(bx, 10 + b, x.hair, tone=2)
    face(x, c)


def face(x: Ctx, c: C):
    b = x.b
    for ex in (13, 16):
        c.px(ex - 1, 12 + b, P.EYE_WHITE)
        c.px(ex, 12 + b, P.EYE)
        c.px(ex, 13 + b, P.EYE)
    c.px(17, 14 + b, x.skin, tone=3)
    c.px(15, 15 + b, P.MOUTH)


# ------------------------------------------------------------------ classi

def vanguard(x: Ctx, c: C):
    b = x.b
    hx, hy = hand_pos(x)
    if x.back:
        shield(x, c, METAL, x.team)
    far_arm(x, c, x.team)
    legs(c, DARK, LEATHER)
    torso(x, c, x.team)
    c.p([(8, 18 + b), (16, 18 + b), (16, 22 + b), (8, 22 + b)], METAL)  # corazza
    head(x, c, show_hair=False)
    # Elmo con cresta.
    c.e(12.5, 10 + b, 7.2, 5.6, METAL, clip=lambda px, py: py <= 9 + b or px <= 8 or (x.back and py <= 16 + b))
    c.r(9, 3 + b, 15, 4 + b, x.team)
    c.r(8, 5 + b, 10, 5 + b, x.team)
    if not x.back:
        face(x, c)
    c.e(16.5, 19.5 + b, 2, 1.6, METAL)  # spallaccio
    near_arm(x, c, x.team)
    sword(c, hx, hy, METAL, GOLD)
    if not x.back:
        shield(x, c, METAL, x.team)


def shield(x: Ctx, c: C, rim: Mat, field: Mat):
    b = x.b
    c.p([(3, 18 + b), (10, 18 + b), (10, 23 + b), (6.5, 27.5 + b), (3, 23 + b)], rim)
    c.p([(4, 19 + b), (9, 19 + b), (9, 23 + b), (6.5, 26 + b), (4, 23 + b)], field)
    c.px(6, 21 + b, GOLD, tone=3)
    c.px(7, 21 + b, GOLD, tone=2)


def sword(c: C, hx: int, hy: int, blade: Mat, hilt: Mat):
    c.r(hx + 2, hy - 13, hx + 3, hy - 2, blade)
    c.px(hx + 2, hy - 14, blade, tone=3)
    c.ln(hx + 2, hy - 12, hx + 2, hy - 3, blade, tone=3)
    c.r(hx, hy - 1, hx + 5, hy - 1, hilt)
    c.r(hx + 2, hy, hx + 3, hy + 1, LEATHER)
    c.px(hx + 2, hy + 2, hilt, tone=3)


def scout(x: Ctx, c: C):
    b = x.b
    hx, hy = hand_pos(x)
    if not x.back:
        c.p([(6, 18 + b), (11, 18 + b), (10, 28 + b), (5, 27 + b)], x.team)  # mantello dietro
        c.r(7, 14 + b, 8, 21 + b, LEATHER)  # faretra
        for ax in (7, 8):
            c.px(ax, 13 + b, WHITE, tone=3)
    far_arm(x, c, GREEN)
    legs(c, LEATHER, LEATHER)
    torso(x, c, GREEN)
    head(x, c)
    # Cappuccio.
    c.e(12.3, 10.6 + b, 7.3, 6.4, x.team,
        clip=lambda px, py: py <= 8 + b or px <= 7 or (px <= 9 and py <= 11 + b) or (x.back and py <= 17 + b))
    c.p([(4, 13 + b), (7, 10 + b), (7, 15 + b)], x.team)  # punta del cappuccio
    if not x.back:
        face(x, c)
    else:
        c.p([(5, 18 + b), (16, 18 + b), (17, 28 + b), (5, 28 + b)], x.team)  # mantello visto da dietro
        c.ln(7, 21 + b, 12, 14 + b, LEATHER)  # faretra in diagonale
        c.ln(8, 21 + b, 13, 14 + b, LEATHER)
        c.px(13, 13 + b, WHITE, tone=3)
        c.px(14, 13 + b, WHITE, tone=3)
    near_arm(x, c, GREEN)
    bow(c, hx, hy)


def bow(c: C, hx: int, hy: int):
    top, bot = hy - 10, hy + 5
    pts = []
    for yy in range(top, bot + 1):
        t = (yy - top) / (bot - top)
        bulge = int(round(2.5 * (1 - (2 * t - 1) ** 2)))
        pts.append((hx + 1 + bulge, yy))
    part = c.new_part()
    for px_, py_ in pts:
        c.put(px_ + OX, py_, WOOD, part)
    c.ln(hx + 1, top, hx + 1, bot, WHITE, tone=2)


def arcanist(x: Ctx, c: C):
    b = x.b
    hx, hy = hand_pos(x)
    far_arm(x, c, PURPLE)
    legs(c, DARK, LEATHER)
    torso(x, c, PURPLE, belt=x.team, long_robe=True)
    c.r(6, 29, 18, 30, x.team)  # orlo
    head(x, c, show_hair=False)
    # Cappuccio ampio che ombreggia il volto.
    c.e(12.4, 10.4 + b, 7.6, 7, PURPLE,
        clip=lambda px, py: py <= 9 + b or px <= 8 or (px <= 9 and py <= 13 + b) or (x.back and py <= 17 + b))
    c.p([(3, 16 + b), (6, 9 + b), (8, 12 + b), (7, 16 + b)], PURPLE)  # punta ricadente
    c.r(9, 9 + b, 17, 9 + b, x.team)  # bordo del cappuccio
    if not x.back:
        for yy in (10, 11):
            for xx in range(10, 18):
                if c.mat[yy + b][xx + OX] is x.skin:
                    c.px(xx, yy + b, x.skin, tone=1)
        face(x, c)
    near_arm(x, c, PURPLE)
    staff(c, hx, hy, WOOD)
    c.e(hx + 1.5, hy - 14.5, 2.8, 2.8, GLOW)  # sfera
    c.px(hx + 1, hy - 15, GLOW, tone=3)
    if x.act:
        for sx, sy in ((hx - 1, hy - 17), (hx + 4, hy - 16), (hx + 3, hy - 19)):
            c.px(sx, sy, GLOW, tone=3)


def staff(c: C, hx: int, hy: int, wood: Mat):
    c.r(hx + 1, hy - 12, hx + 1, hy + 5, wood)


def mender(x: Ctx, c: C):
    b = x.b
    hx, hy = hand_pos(x)
    far_arm(x, c, CREAM)
    legs(c, CREAM, LEATHER)
    torso(x, c, CREAM, belt=GREEN, long_robe=True)
    c.r(6, 29, 18, 30, x.team)
    c.r(11, 18 + b, 13, 28, x.team)  # stola
    head(x, c)
    # Cappello a tesa larga.
    c.e(12.5, 8.6 + b, 9.4, 2.4, CREAM)
    c.e(12.5, 6 + b, 5, 3.4, CREAM, clip=lambda px, py: py <= 7 + b)
    c.r(8, 7 + b, 17, 7 + b, x.team)
    c.px(15, 6 + b, ROSE, tone=3)
    c.px(16, 6 + b, ROSE, tone=2)
    near_arm(x, c, CREAM)
    staff(c, hx, hy, WOOD)
    c.e(hx + 1.5, hy - 13.5, 2.3, 2.3, GOLD)
    c.e(hx + 1.5, hy - 13.5, 1, 1, GOLD, tone=0)
    c.px(hx + 3, hy - 11, GREEN, tone=3)
    c.px(hx + 4, hy - 12, GREEN, tone=2)


def runeblade(x: Ctx, c: C):
    b = x.b
    hx, hy = hand_pos(x)
    # Sciarpa svolazzante dietro.
    c.p([(9, 17 + b), (5, 19 + b), (2, 24 + b), (4, 25 + b), (8, 20 + b)], x.team)
    far_arm(x, c, DARK)
    legs(c, DARK, LEATHER)
    torso(x, c, DARK, belt=GOLD)
    head(x, c)
    c.r(7, 8 + b, 18, 8 + b, x.team)  # fascia
    c.r(8, 17 + b, 16, 18 + b, x.team)  # sciarpa al collo
    c.e(16.5, 19.5 + b, 2, 1.6, METAL)
    near_arm(x, c, DARK)
    sword(c, hx, hy, GLOW, DARK)
    for ry in (hy - 9, hy - 6, hy - 3):
        c.px(hx + 1, ry, GLOW, tone=3)


def warden(x: Ctx, c: C):
    b = x.b
    hx, hy = hand_pos(x)
    if x.back:
        tower_shield(x, c)
    far_arm(x, c, METAL)
    legs(c, METAL, DARK)
    torso(x, c, METAL, belt=DARK)
    c.r(11, 18 + b, 13, 27 + b, x.team)  # tabarro
    c.px(12, 20 + b, GOLD, tone=3)
    head(x, c, show_hair=False)
    # Elmo integrale.
    c.e(12.5, 11.3 + b, 6.9, 6.6, METAL)
    if not x.back:
        c.r(12, 12 + b, 18, 12 + b, DARK, tone=0)
        c.r(14, 12 + b, 14, 15 + b, DARK, tone=0)
    c.r(11, 4 + b, 14, 5 + b, x.team)
    c.e(16.5, 19.5 + b, 2.3, 1.8, METAL)
    near_arm(x, c, METAL)
    c.r(hx + 1, hy - 8, hx + 1, hy + 2, WOOD)  # mazza
    c.e(hx + 1.5, hy - 10, 2.8, 2.6, METAL)
    for sx, sy in ((hx + 1, hy - 13), (hx - 1, hy - 10), (hx + 4, hy - 10)):
        c.px(sx, sy, METAL, tone=3)
    if not x.back:
        tower_shield(x, c)


def tower_shield(x: Ctx, c: C):
    b = x.b
    c.r(2, 15 + b, 9, 28 + b, METAL)
    c.r(3, 16 + b, 8, 27 + b, x.team)
    c.r(5, 18 + b, 6, 25 + b, GOLD)
    c.r(4, 20 + b, 7, 21 + b, GOLD)


CLASSES = {
    "vanguard": vanguard,
    "scout": scout,
    "arcanist": arcanist,
    "mender": mender,
    "runeblade": runeblade,
    "warden": warden,
}


def frame(cls: str, team: int, hair: str, skin: str, view: str, bob: int, act: bool, step: int = 0) -> Image.Image:
    global STEP
    STEP = step
    c = C()
    x = Ctx(
        team=Mat(P.TEAM[team]),
        team_dark=Mat(P.TEAM[team]),
        hair=Mat(P.HAIR[hair]),
        skin=Mat(P.SKIN[skin]),
        b=bob,
        back=(view == "back"),
        act=act,
    )
    CLASSES[cls](x, c)
    return c.render()


def sheet_row(cls: str, team: int, hair: str, skin: str) -> Image.Image:
    """Una riga di fotogrammi nell'ordine di FRAMES."""
    row = Image.new("RGBA", (W * len(FRAMES), H), (0, 0, 0, 0))
    imgs = [
        frame(cls, team, hair, skin, "front", 0, False),
        frame(cls, team, hair, skin, "front", 1, False),
        frame(cls, team, hair, skin, "back", 0, False),
        frame(cls, team, hair, skin, "back", 1, False),
        frame(cls, team, hair, skin, "front", 0, True),
        frame(cls, team, hair, skin, "back", 0, True),
    ]
    ko = imgs[0].rotate(90, expand=True)  # 32x32 ruotato: sdraiato
    ko_cell = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    bbox = ko.getbbox()
    if bbox:
        crop = ko.crop(bbox)
        ko_cell.paste(crop, ((W - crop.width) // 2, H - 2 - crop.height), crop)
    imgs.append(ko_cell)
    for view in ("front", "back"):
        for st in (1, -1):
            # Durante il passo il busto si abbassa di un pixel.
            imgs.append(frame(cls, team, hair, skin, view, 1 if st == 1 else 0, False, st))
    for i, im in enumerate(imgs):
        row.paste(im, (i * W, 0), im)
    return row

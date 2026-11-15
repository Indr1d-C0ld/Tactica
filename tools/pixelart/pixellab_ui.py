"""Cornici dell'interfaccia dai pannelli PixelLab (tools/pixelart/pixellab/ui/).

- window_full.png: finestra con cornice dorata decorata -> ui_window.png
  (ritagliata, angoli di 18 pixel: margine 9-slice 18);
- button_full.png: pannello a bordo fine -> ui_button.png 24x24 (margine 8),
  da cui si ricavano ui_button_hover, _pressed, _disabled e ui_field.

  python3 tools/pixelart/pixellab_ui.py   # scrive in client/assets/override/
"""

import colorsys
from pathlib import Path

from PIL import Image

SRC = Path(__file__).parent / "pixellab" / "ui"
OUT = Path(__file__).resolve().parents[2] / "client" / "assets" / "override"

WINDOW_MARGIN = 18
BUTTON = 24
EDGE = 9  # righe/colonne prese dai bordi del pannello per la cornice del pulsante


def crop(im: Image.Image) -> Image.Image:
    return im.crop(im.getbbox())


def shrink_frame(im: Image.Image, size: int, edge: int) -> Image.Image:
    """Cornice compatta: bordi originali, centro accorciato."""
    w, h = im.size
    out = Image.new("RGBA", (size, size))
    xs = list(range(edge)) + [w // 2] * (size - 2 * edge) + list(range(w - edge, w))
    ys = list(range(edge)) + [h // 2] * (size - 2 * edge) + list(range(h - edge, h))
    src = im.load()
    dst = out.load()
    for y, sy in enumerate(ys):
        for x, sx in enumerate(xs):
            dst[x, y] = src[sx, sy]
    return out


def tone(im: Image.Image, light: float = 1.0, sat: float = 1.0, fill: tuple | None = None) -> Image.Image:
    """Variante: luminosità e saturazione; `fill` sostituisce l'interno blu."""
    out = im.copy()
    px = out.load()
    inner = im.getpixel((im.width // 2, im.height // 2))[:3]
    for y in range(im.height):
        for x in range(im.width):
            r, g, b, a = px[x, y]
            if not a:
                continue
            if fill is not None and sum(abs(c - d) for c, d in zip((r, g, b), inner)) < 30:
                r, g, b = fill
            hh, ll, ss = colorsys.rgb_to_hls(r / 255, g / 255, b / 255)
            r2, g2, b2 = colorsys.hls_to_rgb(hh, min(1.0, ll * light), min(1.0, ss * sat))
            px[x, y] = (int(r2 * 255), int(g2 * 255), int(b2 * 255), a)
    return out


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    win = crop(Image.open(SRC / "window_full.png").convert("RGBA"))
    win.save(OUT / "ui_window.png")
    btn = shrink_frame(crop(Image.open(SRC / "button_full.png").convert("RGBA")), BUTTON, EDGE)
    btn.save(OUT / "ui_button.png")
    tone(btn, light=1.35, sat=1.1).save(OUT / "ui_button_hover.png")
    tone(btn, light=0.75).save(OUT / "ui_button_pressed.png")
    tone(btn, light=0.7, sat=0.25).save(OUT / "ui_button_disabled.png")
    tone(btn, fill=(14, 16, 32)).save(OUT / "ui_field.png")
    print("cornici scritte in", OUT.relative_to(OUT.parents[2]), "- finestra", win.size)


if __name__ == "__main__":
    main()

"""Fondale della battaglia e freccia dell'unità attiva da PixelLab.

  pixellab/scene/backdrop.png   cielo al crepuscolo con montagne (256x144)
  pixellab/scene/marker.png     freccia dorata (32x32)

Il fondale sostituisce backdrop.png; la freccia va nella cella (64, 192) di una
copia di terrain.png (gli altri indicatori restano quelli generati).

  python3 tools/pixelart/pixellab_scene.py
"""

from __future__ import annotations

from pathlib import Path

from PIL import Image

HERE = Path(__file__).parent
SRC = HERE / "pixellab" / "scene"
ASSETS = HERE.parents[1] / "client" / "assets"
DEST = ASSETS / "override"
ARROW_CELL = (64, 192, 96, 224)


def main() -> None:
    DEST.mkdir(parents=True, exist_ok=True)
    Image.open(SRC / "backdrop.png").convert("RGBA").save(DEST / "backdrop.png")
    overlay = Image.open(ASSETS / "terrain.png").convert("RGBA")
    overlay.paste(Image.new("RGBA", (32, 32), (0, 0, 0, 0)), ARROW_CELL[:2])
    overlay.alpha_composite(Image.open(SRC / "marker.png").convert("RGBA"), ARROW_CELL[:2])
    overlay.save(DEST / "terrain.png")
    print("override: backdrop.png, terrain.png")


if __name__ == "__main__":
    main()

"""Anteprima ingrandita degli sprite (per controllo visivo)."""

import sys

from PIL import Image

import units

SCALE = 5


def main(out: str):
    rows = []
    for cls in units.CLASSES:
        for team in (0, 1):
            rows.append(units.sheet_row(cls, team, "castani" if team == 0 else "rossi", "chiara" if team == 0 else "olivastra"))
    w = rows[0].width
    sheet = Image.new("RGBA", (w, units.H * len(rows)), (40, 44, 56, 255))
    for i, r in enumerate(rows):
        sheet.paste(r, (0, i * units.H), r)
    sheet = sheet.resize((sheet.width * SCALE, sheet.height * SCALE), Image.NEAREST)
    sheet.save(out)
    print(out, sheet.size)


if __name__ == "__main__":
    main(sys.argv[1])

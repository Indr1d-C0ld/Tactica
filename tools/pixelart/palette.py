"""Palette originale del progetto: rampe da 4 toni con leggero spostamento di
tinta (ombre più fredde, luci più calde), ispirata alla resa vivace dei
giochi tattici portatili ma con colori propri."""

from pixlib import Mat, ramp

OUTLINE = (27, 22, 38, 255)

SKIN = {
    "chiara": ramp("#7a4434", "#c27a55", "#eaa97f", "#f9d6ae"),
    "olivastra": ramp("#5e3424", "#9a5c3c", "#c9875c", "#e8b48a"),
    "scura": ramp("#3a2019", "#6b3f2c", "#94603f", "#b9835c"),
}

HAIR = {
    "castani": ramp("#2e1c1a", "#5c3527", "#8c5636", "#b87c4c"),
    "biondi": ramp("#5c3e1a", "#a87a2c", "#dcb04a", "#f6dc86"),
    "neri": ramp("#141221", "#272538", "#403e58", "#64627e"),
    "rossi": ramp("#45161a", "#842a22", "#c0452c", "#ea7848"),
    "argento": ramp("#3e4358", "#727a92", "#acb3c6", "#e2e6f0"),
}

TEAM = [
    ramp("#1b275a", "#2c4ea6", "#4a80e2", "#93bdff"),  # squadra 0: blu
    ramp("#4a1420", "#922c2e", "#d24c3a", "#ff8f6a"),  # squadra 1: rosso
]

METAL = ramp("#2c3244", "#5b687e", "#99a6b9", "#e2e9f1")
GOLD = ramp("#55360f", "#9c6719", "#dea52e", "#ffe48a")
LEATHER = ramp("#36221a", "#664026", "#966238", "#c08a52")
WOOD = ramp("#3a281a", "#6c4829", "#9a6c3c", "#c8985c")
CREAM = ramp("#6a5644", "#b4a084", "#e6d9bd", "#fffbf0")
GREEN = ramp("#153822", "#28683a", "#48a058", "#8ed98a")
PURPLE = ramp("#28173e", "#4b2b6e", "#7949a6", "#b68ae0")
DARK = ramp("#221e29", "#3c3645", "#5c5466", "#857b8f")
GLOW_BLUE = ramp("#1a3a7a", "#3b7ce2", "#7ccaff", "#e2f8ff")
GLOW_ROSE = ramp("#5a1a40", "#a8357a", "#e66aa8", "#ffc4e0")
WHITE = ramp("#8a8fa6", "#c4c8d8", "#eef0f6", "#ffffff")
EYE = Mat(ramp("#141221", "#141221", "#141221", "#ffffff"), flat=True, tone=0, outline=False)
EYE_WHITE = Mat(ramp("#f4f0ea", "#f4f0ea", "#f4f0ea", "#f4f0ea"), flat=True, tone=0, outline=False)
EYE_HI = Mat(ramp("#ffffff", "#ffffff", "#ffffff", "#ffffff"), flat=True, tone=3, outline=False)
MOUTH = Mat(ramp("#7a3a34", "#7a3a34", "#7a3a34", "#7a3a34"), flat=True, tone=0, outline=False)

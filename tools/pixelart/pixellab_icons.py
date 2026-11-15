"""Icone degli oggetti generate con PixelLab (Pixen, 32x32, sfondo trasparente).

Una icona per ogni base degli oggetti generati, una per ogni leggendario, più
stivali e corno (oggetti del catalogo classico) e il sacchetto del bottino.
Le immagini scaricate restano in tools/pixelart/pixellab/icons/ (sono la
sorgente: non si rigenerano a ogni build); `atlas` compone l'atlante del client.

  python3 tools/pixelart/pixellab_icons.py generate   # genera quelle mancanti (serve la chiave PixelLab)
  python3 tools/pixelart/pixellab_icons.py atlas      # scrive client/assets/items.png e items.json

La chiave si legge dalla variabile PIXELLAB_TOKEN.
"""

from __future__ import annotations

import json
import os
import sys
import time
import urllib.request
from pathlib import Path

HERE = Path(__file__).parent
OUT = HERE / "pixellab" / "icons"
ROOT = HERE.parents[1]
API = "https://api.pixellab.ai/mcp"

STYLE = ("single RPG inventory item icon, centered, three-quarter view, clean dark outline, "
         "soft volume shading, rich 16-bit fantasy tactics pixel art style")

ICONS: dict[str, str] = {
    # Armi
    "spa": "a straight steel longsword with a cross guard and leather grip",
    "sgr": "a huge two-handed greatsword with a wide steel blade",
    "lan": "a long spear with a steel leaf-shaped spearhead and wooden shaft",
    "asc": "a battle axe with a single crescent steel blade and wooden haft",
    "maz": "a flanged steel mace with a short wooden handle",
    "pug": "a short curved dagger with a steel blade and wrapped hilt",
    "sci": "a curved steel sabre with a golden hand guard",
    "arc": "a small curved wooden hand bow with a thin string, no crossbow stock, held upright",
    "arl": "a tall wooden longbow drawn vertically with a taut string and leather grip",
    "bal": "a wooden crossbow with a steel bow arm, seen from above",
    "bac": "a short slender magic wand of carved wood with a small glowing tip",
    "ver": "an ornate metal sceptre rod topped with a blue crystal",
    "bas": "a tall wooden quarterstaff with a curled top and cloth wraps",
    "lru": "a sword with glowing cyan runes engraved along the blade",
    "fla": "a spiked iron ball flail on a chain with a wooden handle",
    "tom": "a thick closed spellbook with a leather cover and a metal clasp",
    # Corazze
    "tun": "a simple green cloth tunic with a rope belt",
    "ves": "an empty long blue mage robe with gold trim laid flat, no person, garment only",
    "sai": "an empty plain brown monk robe with a rope belt laid flat, no person, garment only",
    "giu": "a padded leather jerkin vest",
    "gia": "an empty studded leather armor vest on its own, no person, armor piece only",
    "usb": "a chainmail hauberk shirt",
    "bri": "a brigandine armor of cloth with riveted metal plates",
    "cpi": "a heavy steel plate cuirass breastplate with pauldrons",
    # Copricapi
    "elm": "a steel knight helmet with a visor and a small crest",
    "bar": "a rounded steel barbute helmet with a T-shaped face opening",
    "cpc": "an empty green cloth hood cowl on its own, no face, no person",
    "cpl": "a pointed wizard hat with a wide brim",
    "cer": "a thin golden circlet with a small gem",
    "ben": "a red cloth headband with trailing ties",
    # Accessori
    "ane": "a gold ring with a red gemstone",
    "amu": "a silver amulet pendant with a blue gem on a chain",
    "cin": "a leather belt with a brass buckle and small pouch",
    "brc": "a wide metal bracer armband with engravings",
    "man": "a folded travelling cloak with a clasp",
    "spl": "an ornate brooch pin with a green gem",
    "tal": "a carved bone and wood talisman charm on a cord",
    # Leggendari
    "legend_alba": "a legendary straight longsword with a white-gold blade and a golden sun-shaped cross guard, soft warm glow",
    "legend_zanna": "a legendary short dagger whose blade is a long curved white serpent fang, silver hilt, green venom drop",
    "legend_bastione": "a legendary massive ornate steel breastplate with gold engravings of a tower, glowing faintly",
    "legend_sigillo": "a legendary golden seal amulet with an engraved pact symbol and glowing runes",
    "legend_brace": "a legendary longbow made of dark wood with glowing ember cracks and fiery string",
    "legend_nevi": "a legendary ice sceptre of frozen crystal with frost mist, pale blue glow",
    "legend_tuono": "a legendary flail with a storm-charged iron ball crackling with yellow lightning",
    "legend_radici": "a legendary crown made of living twisted roots and small green leaves",
    "legend_grigio": "a legendary folded grey travelling cloak with a silver star clasp, empty garment, no person",
    "legend_sorgente": "a legendary flowing white and aqua robe with water-like shimmering trim",
    "legend_fauce": "a legendary black obsidian battle axe with a glowing orange molten edge, no text",
    "legend_cielo": "a legendary steel helmet with a lightning-shaped crack glowing blue",
    "legend_eco": "a legendary dark runic sword with purple glowing runes and a ghostly echo aura",
    "legend_lunaverde": "a legendary silver ring with a glowing pale green moonstone, isolated on transparent background",
    "legend_sabbie": "a legendary sand-coloured leather belt with turquoise gems and gold buckle",
    "legend_cenere": "a legendary black grimoire with burning ember pages and a skull clasp",
    # Catalogo classico e bottino
    "boots": "a pair of light leather boots with small wings",
    "horn": "a curved war horn made of bone with metal bands",
    "bag": "a small tied loot sack pouch with a gold coin peeking out",
    # Consumabili
    "use_pozione": "a small round glass potion flask with red healing liquid and a cork",
    "use_etere": "a slender glass vial with glowing blue mana liquid and a silver cap",
    "use_rimedio": "a small clay jar of green herbal remedy salve with a cloth lid",
    "use_essenza": "a crystal phial with golden glowing life essence and a feather charm",
    "use_elisir": "an ornate golden bottle with shimmering rainbow elixir and a jewelled stopper",
    "use_bomba": "a round black iron bomb with a lit fuse and orange ember cracks",
    "use_gelo": "a frosted glass vial with icy pale blue liquid and frost crystals",
    "use_tossina": "a green glass vial with bubbling toxic poison and a skull label",
    "use_sonno": "a small purple pouch of glittering sleep powder with sparkles",
    "use_p_fiamma": "a rolled parchment scroll tied with a ribbon and wax seal, with a glowing red flame rune symbol",
    "use_p_gelo": "a rolled parchment scroll tied with a ribbon and wax seal, with a glowing light blue snowflake rune symbol",
    "use_p_folgore": "a rolled parchment scroll tied with a ribbon and wax seal, with a glowing yellow lightning bolt rune symbol",
    "use_p_vampa": "a rolled parchment scroll tied with a ribbon and wax seal, with a glowing large orange fireball rune symbol",
    "use_p_luce": "a rolled parchment scroll tied with a ribbon and wax seal, with a glowing white star rune symbol",
    "use_p_giudizio": "a rolled parchment scroll tied with a ribbon and wax seal, with a glowing golden sun rune symbol",
    "use_p_catene": "a rolled parchment scroll tied with a ribbon and wax seal, with a glowing grey chain links rune symbol",
    "use_p_torpore": "a rolled parchment scroll tied with a ribbon and wax seal, with a glowing purple crescent moon rune symbol",
    "use_p_cura": "a rolled parchment scroll tied with a ribbon and wax seal, with a glowing green cross rune symbol",
    "use_p_cura_m": "a rolled parchment scroll tied with a ribbon and wax seal, with a glowing bright green double cross rune symbol",
    "use_p_santuario": "a rolled parchment scroll tied with a ribbon and wax seal, with a glowing white dove rune symbol",
    "use_p_egida": "a rolled parchment scroll tied with a ribbon and wax seal, with a glowing silver shield rune symbol",
    "use_p_germoglio": "a rolled parchment scroll tied with a ribbon and wax seal, with a glowing green sprout leaf rune symbol",
    "use_p_purif": "a rolled parchment scroll tied with a ribbon and wax seal, with a glowing clear water drop rune symbol",
    "use_p_slancio": "a rolled parchment scroll tied with a ribbon and wax seal, with a glowing orange wind swirl rune symbol",
}


def call(tool: str, args: dict) -> str:
    token = os.environ["PIXELLAB_TOKEN"]
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
                       "params": {"name": tool, "arguments": args}}).encode()
    req = urllib.request.Request(API, data=body, headers={
        "Authorization": f"Bearer {token}", "Content-Type": "application/json",
        "Accept": "application/json, text/event-stream"})
    text = urllib.request.urlopen(req, timeout=300).read().decode()
    if text.lstrip().startswith("event") or "data:" in text[:80]:
        text = "\n".join(line[5:] for line in text.splitlines() if line.startswith("data:"))
    d = json.loads(text)
    if "error" in d:
        raise RuntimeError(d["error"])
    return "\n".join(c.get("text", "") for c in d["result"].get("content", []) if c.get("type") == "text")


def generate() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    todo = [k for k in ICONS if not (OUT / f"{k}.png").exists()]
    pending: dict[str, str] = {}
    while todo or pending:
        # Al massimo 4 lavori alla volta, per lasciare posto ad altri usi dell'account.
        while todo and len(pending) < 4:
            k = todo[0]
            out = call("create_image_pixen", {"description": f"{ICONS[k]}, {STYLE}",
                                              "width": 32, "height": 32, "no_background": True})
            if "rate limit" in out or "job slots" in out:
                break
            todo.pop(0)
            job = next(line.split(": ")[1] for line in out.splitlines() if line.startswith("job_id:"))
            pending[k] = job
            print("accodata", k, flush=True)
        time.sleep(8)
        for k, job in list(pending.items()):
            st = call("get_image", {"job_id": job})
            if "status: completed" in st:
                data = urllib.request.urlopen(f"{API}/images/{job}/download", timeout=120).read()
                (OUT / f"{k}.png").write_bytes(data)
                del pending[k]
                print("scaricata", k, flush=True)
            elif "failed" in st.splitlines()[0]:
                del pending[k]
                todo.append(k)
                print("fallita, riprovo", k, flush=True)


def atlas() -> None:
    from PIL import Image
    keys = [k for k in ICONS if (OUT / f"{k}.png").exists()]
    cols = 8
    rows = (len(keys) + cols - 1) // cols
    sheet = Image.new("RGBA", (cols * 32, rows * 32), (0, 0, 0, 0))
    index = {}
    for i, k in enumerate(keys):
        im = Image.open(OUT / f"{k}.png").convert("RGBA")
        if im.size != (32, 32):
            im = im.resize((32, 32), Image.NEAREST)
        sheet.alpha_composite(im, ((i % cols) * 32, (i // cols) * 32))
        index[k] = i
    dest = ROOT / "client" / "assets"
    sheet.save(dest / "items.png")
    (dest / "items.json").write_text(json.dumps({"cell": 32, "columns": cols, "icons": index}, indent=1))
    print(f"client/assets/items.png: {len(keys)} icone")


if __name__ == "__main__":
    cmd = sys.argv[1] if len(sys.argv) > 1 else "atlas"
    {"generate": generate, "atlas": atlas}[cmd]()

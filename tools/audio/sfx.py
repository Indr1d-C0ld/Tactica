"""Effetti sonori originali di Tactica (sintesi FM e rumore filtrato, fm.py).

  python3 tools/audio/sfx.py   # scrive client/assets/audio/sfx_*.wav (mono, 32 kHz)
"""

from __future__ import annotations

import wave
from pathlib import Path

import numpy as np

import fm
from fm import SR, Patch, drum, fm_note, highpass, midi, noise

OUT = Path(__file__).resolve().parents[2] / "client" / "assets" / "audio"


# ------------------------------------------------------------------ mattoni

def t_(sec: float) -> np.ndarray:
    return np.arange(int(sec * SR)) / SR


def decay(sec: float, tau: float) -> np.ndarray:
    return np.exp(-t_(sec) / tau)


def sweep(f0: float, f1: float, sec: float, kind: str = "sine", curve: float = 1.0) -> np.ndarray:
    t = t_(sec)
    f = f0 + (f1 - f0) * (t / sec) ** curve
    ph = 2 * np.pi * np.cumsum(f) / SR
    return fm.wave(kind, ph)


def nz(sec: float, seed: int, color: float = 0.0, hp: float = 0.0) -> np.ndarray:
    """Rumore: `color` > 0 lo scurisce (passa-basso), `hp` > 0 lo schiarisce."""
    x = noise(int(sec * SR), seed)
    if color:
        x = fm._lowpass(x, color)
    if hp:
        x = highpass(x, hp)
    return x


def mix(*parts: tuple[np.ndarray, float, float]) -> np.ndarray:
    """(segnale, volume, ritardo in secondi)."""
    n = max(int(d * SR) + len(x) for x, _, d in parts)
    out = np.zeros(n, dtype=np.float32)
    for x, v, d in parts:
        i = int(d * SR)
        out[i:i + len(x)] += x * v
    return out


def tone(name: str | float, sec: float, patch: Patch, vel: float = 1.0) -> np.ndarray:
    m = midi(name) if isinstance(name, str) else name
    return fm_note(patch, m, sec, vel)


def arpeggio(notes: list[str], step: float, patch: Patch, length: float = 0.25, vel: float = 0.8) -> np.ndarray:
    return mix(*[(tone(n, length, patch, vel), 1.0, i * step) for i, n in enumerate(notes)])


def finish(x: np.ndarray, peak: float = 0.85, fade: float = 0.06, max_len: float = 2.5) -> np.ndarray:
    x = x - np.mean(x)
    # Taglia la coda sotto i -50 dB (e comunque oltre `max_len` secondi).
    loud = np.nonzero(np.abs(x) > np.max(np.abs(x)) * 0.003)[0]
    end = min(int(loud[-1]) + 1 if len(loud) else len(x), int(max_len * SR))
    x = x[:end].copy()
    f = int(fade * SR)
    if f and len(x) > f:
        x[-f:] *= np.linspace(1, 0, f)
        x[: min(32, len(x))] *= np.linspace(0, 1, min(32, len(x)))
    m = np.max(np.abs(x)) + 1e-9
    return (x / m * peak).astype(np.float32)


def room(x: np.ndarray, wet: float = 0.2, sec: float = 0.8) -> np.ndarray:
    st = np.stack([x, x], axis=1)
    pad = np.zeros((int(sec * SR), 2), dtype=np.float32)
    y = fm.reverb(np.concatenate([st, pad]), sec, wet, circular=False)
    return y.mean(axis=1)


BELL = Patch(ratio=3.5, index=2.0, index_end=0.2, index_decay=0.3, a=0.001, d=0.5, s=0.0, r=0.4, gain=0.6)
CHIME = Patch(ratio=7.0, index=1.2, index_end=0.1, index_decay=0.15, a=0.001, d=0.4, s=0.0, r=0.3, gain=0.5)
PLUCK = Patch(ratio=2.0, index=2.0, index_end=0.1, index_decay=0.05, a=0.001, d=0.12, s=0.0, r=0.06)
SOFT = Patch(ratio=1.0, index=0.5, index_end=0.2, a=0.005, d=0.1, s=0.2, r=0.05)
METAL = Patch(ratio=1.41, index=6.0, index_end=1.0, index_decay=0.08, a=0.001, d=0.25, s=0.0, r=0.3, gain=0.7)
BRASS = fm.INSTR["brass"]
CHOIR = fm.INSTR["choir"]


# ------------------------------------------------------------------ interfaccia

def ui_click(): return finish(mix((tone("E6", 0.03, PLUCK), 1, 0), (nz(0.01, 1, hp=0.8), 0.3, 0)), 0.5)
def ui_hover(): return finish(tone("B6", 0.02, SOFT, 0.6), 0.22)
def ui_confirm(): return finish(arpeggio(["C6", "G6"], 0.06, CHIME, 0.15), 0.6)
def ui_cancel(): return finish(arpeggio(["G5", "D5"], 0.06, PLUCK, 0.12), 0.55)
def ui_error(): return finish(mix((tone("A3", 0.12, Patch(ratio=1, index=3, index_end=3, car_wave="square", a=0.002, d=0.1, s=0.8, r=0.03)), 1, 0),
                                  (tone("A3", 0.12, Patch(ratio=1, index=3, index_end=3, car_wave="square", a=0.002, d=0.1, s=0.8, r=0.03)), 1, 0.16)), 0.5)
def ui_tab(): return finish(mix((nz(0.05, 2, hp=0.6) * decay(0.05, 0.015), 0.6, 0), (tone("A5", 0.04, PLUCK), 0.6, 0)), 0.45)


def equip():
    clink = mix((tone(88, 0.3, METAL, 0.8), 1, 0), (tone(95, 0.25, METAL, 0.6), 0.7, 0.04))
    leather = nz(0.12, 3, color=0.6) * decay(0.12, 0.04)
    return finish(mix((clink, 0.8, 0.05), (leather, 0.6, 0)), 0.7)


def unequip(): return finish(mix((nz(0.15, 4, color=0.5) * decay(0.15, 0.05), 0.8, 0), (tone(76, 0.2, METAL, 0.5), 0.4, 0.05)), 0.6)


def buy():
    coins = mix(*[(tone(96 + (k % 3) * 3, 0.15, CHIME, 0.8), 1, k * 0.045) for k in range(5)])
    return finish(mix((coins, 1, 0), (arpeggio(["C6", "E6", "G6"], 0.07, BELL, 0.3), 0.5, 0.15)), 0.7)


def sell(): return finish(mix(*[(tone(100 - k * 2, 0.12, CHIME, 0.8), 1, k * 0.05) for k in range(4)]), 0.65)


def forge():
    hits = [(mix((tone(84, 0.5, METAL), 1, 0), (nz(0.06, 5 + k, hp=0.5) * decay(0.06, 0.02), 0.6, 0)), 1 - 0.15 * k, k * 0.32) for k in range(3)]
    return finish(room(mix(*hits), 0.25), 0.8)


def brew():
    bubbles = mix(*[(sweep(300 + 140 * (k % 4), 900 + 120 * k, 0.07) * decay(0.07, 0.03), 0.6, k * 0.07) for k in range(9)])
    return finish(mix((bubbles, 1, 0), (arpeggio(["E5", "G#5", "B5", "E6"], 0.08, CHIME, 0.4), 0.5, 0.45)), 0.65)


def enchant():
    shimmer = mix(*[(tone(84 + k * 2, 0.6, CHIME, 0.6), 1, k * 0.05) for k in range(8)])
    return finish(room(mix((shimmer, 1, 0), (sweep(400, 1600, 0.6) * decay(0.6, 0.3), 0.25, 0)), 0.4, 1.2), 0.7)


def salvage():
    crunch = mix(*[(nz(0.05, 10 + k, color=0.3) * decay(0.05, 0.015), 1, k * 0.06) for k in range(5)])
    return finish(mix((crunch, 1, 0), (tone(70, 0.3, METAL, 0.5), 0.5, 0.02)), 0.7)


def recruit():
    return finish(room(mix((arpeggio(["G4", "C5", "E5", "G5"], 0.09, BRASS, 0.3, 0.9), 1, 0),
                           (drum("snare") * 0.4, 1, 0)), 0.25), 0.75)


def class_change():
    whoosh = nz(0.5, 20, color=0.4) * np.sin(np.linspace(0, np.pi, int(0.5 * SR)))
    return finish(room(mix((whoosh, 0.5, 0), (arpeggio(["C5", "F5", "A5", "C6", "F6"], 0.07, CHIME, 0.5), 1, 0.2)), 0.35, 1.0), 0.7)


def turn(): return finish(room(arpeggio(["G5", "D6"], 0.08, BELL, 0.4, 0.7), 0.2), 0.45)


# ------------------------------------------------------------------ passi

def step(color: float, seed: int, body: float = 0.0, splash: float = 0.0):
    x = nz(0.09, seed, color=color) * decay(0.09, 0.025)
    if body:
        x = mix((x, 1, 0), (np.sin(2 * np.pi * body * t_(0.06)) * decay(0.06, 0.02), 0.6, 0))
    if splash:
        x = mix((x, 0.7, 0), (nz(0.18, seed + 1, hp=0.6) * decay(0.18, 0.06), splash, 0.01))
    return finish(x, 0.35)


def step_grass(): return step(0.6, 30)
def step_stone(): return step(0.15, 31, body=180)
def step_sand(): return step(0.75, 32)
def step_water(): return step(0.4, 33, splash=0.8)
def step_thicket(): return finish(mix((step(0.5, 34), 1, 0), (nz(0.2, 35, hp=0.4) * decay(0.2, 0.08), 0.4, 0.02)), 0.35)


# ------------------------------------------------------------------ armi e colpi

def whoosh(sec: float, seed: int, f0: float, f1: float):
    env = np.sin(np.linspace(0, np.pi, int(sec * SR))) ** 2
    return nz(sec, seed, hp=0.3) * env * 0.6 + sweep(f0, f1, sec) * env * 0.2


def slash(): return finish(mix((whoosh(0.16, 40, 1800, 600), 1, 0), (tone(98, 0.15, METAL, 0.4), 0.35, 0.1)), 0.7)


def slash_heavy():
    return finish(room(mix((whoosh(0.28, 41, 900, 200), 1, 0), (drum("kick"), 0.8, 0.2), (tone(80, 0.3, METAL, 0.6), 0.5, 0.2)), 0.2), 0.8)


def pierce(): return finish(mix((whoosh(0.12, 42, 2200, 1200), 1, 0), (nz(0.04, 43, hp=0.7) * decay(0.04, 0.01), 0.8, 0.11)), 0.7)


def blunt(): return finish(mix((whoosh(0.14, 44, 500, 200), 0.7, 0), (drum("kick"), 1, 0.12), (drum("snare") * 0.4, 1, 0.12)), 0.8)


def arrow():
    twang = tone("E3", 0.25, Patch(ratio=1, index=3, index_end=0.3, index_decay=0.03, a=0.001, d=0.2, s=0, r=0.1))
    return finish(mix((twang, 0.8, 0), (whoosh(0.18, 45, 3000, 1500), 0.6, 0.04)), 0.65)


def hit(): return finish(mix((drum("snare"), 0.7, 0), (drum("kick"), 0.7, 0)), 0.75)


def hit_heavy(): return finish(room(mix((drum("kick"), 1, 0), (drum("snare"), 0.8, 0), (drum("crash") * 0.3, 1, 0.02)), 0.25), 0.85)


def miss(): return finish(whoosh(0.22, 46, 1200, 400), 0.55)


def block(): return finish(room(mix((tone(91, 0.4, METAL), 1, 0), (tone(98, 0.3, METAL, 0.7), 0.6, 0.005)), 0.2), 0.7)


def ko():
    fall = mix((sweep(400, 60, 0.4, "square") * decay(0.4, 0.15), 0.3, 0), (drum("kick"), 1, 0.35),
               (nz(0.3, 47, color=0.5) * decay(0.3, 0.1), 0.6, 0.35))
    return finish(room(fall, 0.2), 0.8)


# ------------------------------------------------------------------ magia

def fire():
    roar = nz(0.7, 50, color=0.55) * np.minimum(1, t_(0.7) / 0.08) * decay(0.7, 0.3)
    crackle = mix(*[(nz(0.015, 60 + k, hp=0.8), 0.6, 0.05 + k * 0.07) for k in range(8)])
    return finish(room(mix((roar, 1, 0), (crackle, 0.6, 0), (sweep(200, 90, 0.5) * decay(0.5, 0.2), 0.4, 0)), 0.2), 0.8)


def ice():
    shards = mix(*[(tone(96 + (k * 5) % 12, 0.3, CHIME, 0.7), 1, k * 0.04) for k in range(7)])
    crack = nz(0.08, 70, hp=0.7) * decay(0.08, 0.02)
    return finish(room(mix((shards, 0.8, 0.05), (crack, 0.8, 0)), 0.35, 1.0), 0.7)


def thunder():
    zap = nz(0.12, 80, hp=0.5) * decay(0.12, 0.03)
    boom = nz(1.2, 81, color=0.85) * decay(1.2, 0.35)
    return finish(room(mix((zap, 1, 0), (sweep(2000, 100, 0.15, "square") * decay(0.15, 0.05), 0.3, 0), (boom, 1.2, 0.05)), 0.3, 1.4), 0.85)


def holy(): return finish(room(mix((tone("A4", 0.8, CHOIR, 0.8), 1, 0), (tone("E5", 0.8, CHOIR, 0.7), 1, 0), (arpeggio(["A5", "C#6", "E6", "A6"], 0.06, BELL, 0.6), 0.6, 0.15)), 0.45, 1.6), 0.75)


def heal(): return finish(room(arpeggio(["C5", "E5", "G5", "C6", "E6"], 0.07, CHIME, 0.5), 0.4, 1.2), 0.65)


def buff(): return finish(room(mix((arpeggio(["G4", "B4", "D5", "G5"], 0.05, BELL, 0.4), 1, 0), (sweep(300, 900, 0.4) * decay(0.4, 0.2), 0.2, 0)), 0.3), 0.65)


def debuff(): return finish(room(mix((arpeggio(["F5", "C#5", "A#4", "F#4"], 0.07, BELL, 0.4), 1, 0), (sweep(800, 200, 0.4) * decay(0.4, 0.2), 0.25, 0)), 0.3), 0.65)


def drain():
    suck = sweep(1200, 200, 0.5, "abs") * np.minimum(1, t_(0.5) / 0.2) * decay(0.5, 0.3)
    return finish(room(mix((suck, 0.5, 0), (nz(0.5, 90, color=0.6) * decay(0.5, 0.2), 0.4, 0), (tone("E4", 0.4, CHOIR, 0.6), 0.6, 0.1)), 0.3), 0.7)


def magic(): return finish(room(mix((sweep(200, 1400, 0.35, "half") * decay(0.35, 0.2), 0.4, 0), (arpeggio(["D5", "A5", "D6"], 0.05, CHIME, 0.3), 0.8, 0.05)), 0.3), 0.6)


# ------------------------------------------------------------------ status

def poison(): return finish(mix(*[(sweep(200 + 60 * k, 120, 0.12) * decay(0.12, 0.05), 0.7, k * 0.09) for k in range(5)]), 0.6)
def sleep(): return finish(room(arpeggio(["E5", "C5", "A4"], 0.18, Patch(ratio=1, index=0.6, index_end=0.3, a=0.05, d=0.4, s=0.3, r=0.3, vibrato=0.3), 0.4), 0.35, 1.0), 0.55)
def silence(): return finish(mix((tone("A5", 0.15, SOFT), 1, 0), (nz(0.4, 100, color=0.5) * np.linspace(1, 0, int(0.4 * SR)), 0.4, 0.1)), 0.5)
def slow(): return finish(sweep(500, 120, 0.5, "abs") * decay(0.5, 0.4), 0.55)
def haste(): return finish(mix((sweep(300, 2000, 0.25, "abs") * decay(0.25, 0.2), 0.6, 0), (arpeggio(["C6", "D6", "E6"], 0.04, CHIME, 0.15), 0.6, 0.15)), 0.6)
def root(): return finish(mix((nz(0.35, 110, color=0.4) * decay(0.35, 0.15), 0.8, 0), (drum("tom"), 0.7, 0.05)), 0.6)
def regen(): return finish(room(arpeggio(["G5", "B5", "D6"], 0.12, CHIME, 0.3, 0.6), 0.35), 0.5)
def aegis(): return finish(room(mix((tone(88, 0.5, METAL, 0.7), 1, 0), (tone(95, 0.5, BELL, 0.6), 0.6, 0.03)), 0.3), 0.65)
def burden(): return finish(mix((drum("kick"), 1, 0), (sweep(300, 60, 0.4, "square") * decay(0.4, 0.2), 0.25, 0.02), (tone(55, 0.4, METAL, 0.5), 0.4, 0.05)), 0.75)


def flight():
    flaps = mix(*[(nz(0.09, 120 + k, color=0.5) * np.sin(np.linspace(0, np.pi, int(0.09 * SR))), 0.7, k * 0.13) for k in range(4)])
    return finish(mix((flaps, 1, 0), (sweep(400, 1200, 0.5) * decay(0.5, 0.3), 0.15, 0)), 0.6)


def cleanse(): return finish(room(mix((nz(0.4, 130, hp=0.7) * decay(0.4, 0.15), 0.4, 0), (arpeggio(["E6", "B6"], 0.08, CHIME, 0.4), 0.7, 0.05)), 0.35), 0.6)
def mp(): return finish(room(arpeggio(["D5", "F#5", "A5", "D6"], 0.06, Patch(ratio=2, index=1.5, index_end=0.5, a=0.002, d=0.3, s=0.1, r=0.3), 0.3), 0.3), 0.6)


# ------------------------------------------------------------------ oggetti e situazioni

def potion():
    glugs = mix(*[(sweep(280 + 30 * k, 180, 0.07, "sine") * decay(0.07, 0.03), 0.8, k * 0.11) for k in range(4)])
    return finish(mix((glugs, 1, 0), (arpeggio(["C6", "G6"], 0.06, CHIME, 0.2), 0.4, 0.45)), 0.6)


def throw(): return finish(whoosh(0.3, 140, 900, 300), 0.55)


def bomb():
    boom = nz(1.0, 141, color=0.8) * decay(1.0, 0.25)
    return finish(room(mix((drum("kick"), 1, 0), (boom, 1.2, 0), (nz(0.15, 142, hp=0.4) * decay(0.15, 0.04), 0.6, 0)), 0.2), 0.9)


def scroll():
    paper = nz(0.25, 150, hp=0.5) * np.sin(np.linspace(0, np.pi, int(0.25 * SR))) * 0.5
    return finish(mix((paper, 1, 0), (magic(), 0.8, 0.15)), 0.65)


def revive():
    return finish(room(mix((tone("C4", 1.4, CHOIR, 0.7), 1, 0), (tone("G4", 1.4, CHOIR, 0.7), 1, 0.15),
                           (tone("E5", 1.2, CHOIR, 0.7), 1, 0.3), (arpeggio(["C6", "E6", "G6", "C7"], 0.1, BELL, 0.6), 0.6, 0.6)), 0.5, 2.0), 0.75)


def loot_drop(): return finish(room(mix((arpeggio(["E6", "B6", "E7"], 0.05, CHIME, 0.3), 1, 0), (drum("tambourine"), 0.4, 0)), 0.3), 0.55)


def loot_pick():
    coins = mix(*[(tone(98 + k * 2, 0.12, CHIME, 0.8), 1, k * 0.04) for k in range(4)])
    return finish(room(mix((coins, 1, 0), (arpeggio(["G5", "C6", "E6", "G6"], 0.07, BELL, 0.35), 0.6, 0.12)), 0.25), 0.7)


def statute():
    gong = tone(43, 2.0, Patch(ratio=1.4, index=4, index_end=0.6, index_decay=0.6, a=0.003, d=1.5, s=0.0, r=1.0))
    return finish(room(gong, 0.4, 1.8), 0.8)


def suspend(): return finish(mix((statute(), 0.6, 0), (arpeggio(["E4", "C4"], 0.25, BRASS, 0.4), 0.6, 0.1)), 0.75)


def expel():
    return finish(room(mix((statute(), 0.8, 0), (drum("tom"), 1, 0.1), (drum("tom"), 1, 0.3), (drum("kick"), 1, 0.5),
                           (arpeggio(["D4", "A3", "D3"], 0.2, BRASS, 0.5), 0.6, 0.1)), 0.3), 0.85)


def levelup(): return finish(room(mix((arpeggio(["C5", "E5", "G5", "C6", "E6", "G6"], 0.06, BRASS, 0.3, 0.8), 1, 0), (drum("crash") * 0.25, 1, 0.3)), 0.25), 0.75)


def mastery(): return finish(room(mix((arpeggio(["A4", "C#5", "E5", "A5"], 0.1, CHOIR, 0.6, 0.7), 1, 0), (arpeggio(["A5", "E6", "A6"], 0.12, BELL, 0.5), 0.6, 0.3)), 0.4, 1.5), 0.75)


def lava(): return finish(mix((nz(0.6, 160, color=0.3) * decay(0.6, 0.3), 0.7, 0), (fire(), 0.5, 0)), 0.65)


SOUNDS = [
    ui_click, ui_hover, ui_confirm, ui_cancel, ui_error, ui_tab, equip, unequip, buy, sell, forge, brew,
    enchant, salvage, recruit, class_change, turn,
    step_grass, step_stone, step_sand, step_water, step_thicket,
    slash, slash_heavy, pierce, blunt, arrow, hit, hit_heavy, miss, block, ko,
    fire, ice, thunder, holy, heal, buff, debuff, drain, magic,
    poison, sleep, silence, slow, haste, root, regen, aegis, burden, flight, cleanse, mp,
    potion, throw, bomb, scroll, revive, loot_drop, loot_pick, statute, suspend, expel, levelup, mastery, lava,
]


def write_wav(path: Path, x: np.ndarray) -> None:
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes((np.clip(x, -1, 1) * 32767).astype("<i2").tobytes())


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    total = 0
    for fn in SOUNDS:
        x = fn()
        assert np.isfinite(x).all() and len(x) > 0, fn.__name__
        write_wav(OUT / f"sfx_{fn.__name__}.wav", x)
        total += len(x)
    print(f"{len(SOUNDS)} effetti, {total / SR:.1f} s in tutto")


if __name__ == "__main__":
    main()

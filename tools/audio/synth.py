"""Musiche ed effetti sonori originali in stile chiptune, generati da codice.

Voci: due onde quadre (melodia e armonia), un'onda triangolare (basso) e
rumore (percussioni). Le melodie nascono da progressioni di accordi e da
regole semplici (note dell'accordo sui tempi forti, note di passaggio sui
deboli) con un generatore deterministico: ogni esecuzione produce gli stessi
file.

Uso: python3 tools/audio/synth.py   (scrive in client/assets/audio/)
"""

from __future__ import annotations

import math
import random
import struct
import wave
from pathlib import Path

RATE = 22050
OUT = Path(__file__).resolve().parents[2] / "client" / "assets" / "audio"

NOTE = {"C": 0, "C#": 1, "D": 2, "D#": 3, "E": 4, "F": 5, "F#": 6, "G": 7, "G#": 8, "A": 9, "A#": 10, "B": 11}


def midi(name: str) -> int:
    n, o = name[:-1], int(name[-1])
    return 12 * (o + 1) + NOTE[n]


def freq(m: int) -> float:
    return 440.0 * 2 ** ((m - 69) / 12)


# ------------------------------------------------------------------ voci

def square(f: float, t: float, duty: float) -> float:
    return 1.0 if (t * f) % 1.0 < duty else -1.0


def triangle(f: float, t: float) -> float:
    p = (t * f) % 1.0
    return 4 * p - 1 if p < 0.5 else 3 - 4 * p


class Noise:
    def __init__(self, seed: int):
        self.r = random.Random(seed)

    def __call__(self) -> float:
        return self.r.uniform(-1, 1)


def env(i: int, n: int, attack: float = 0.005, release: float = 0.08, sustain: float = 0.75) -> float:
    t = i / RATE
    dur = n / RATE
    if t < attack:
        return t / attack
    if t > dur - release:
        return max(0.0, (dur - t) / release) * sustain
    # Leggero decadimento verso il sustain.
    return sustain + (1 - sustain) * math.exp(-(t - attack) * 12)


def mix_note(buf: list[float], start: int, n: int, f: float, vol: float, kind: str, duty: float = 0.25,
             vibrato: float = 0.0, release: float = 0.08):
    for i in range(n):
        j = start + i
        if j >= len(buf):
            break
        t = i / RATE
        ff = f * (1 + vibrato * math.sin(2 * math.pi * 5.5 * t) * min(1.0, t * 3))
        v = square(ff, t, duty) if kind == "sq" else triangle(ff, t)
        buf[j] += v * vol * env(i, n, release=release)


def kick(buf, start, vol=0.5):
    n = int(RATE * 0.12)
    for i in range(n):
        if start + i >= len(buf):
            break
        t = i / RATE
        f = 120 * math.exp(-t * 30) + 45
        buf[start + i] += math.sin(2 * math.pi * f * t) * vol * math.exp(-t * 25)


def snare(buf, start, noise, vol=0.3):
    n = int(RATE * 0.1)
    for i in range(n):
        if start + i >= len(buf):
            break
        t = i / RATE
        buf[start + i] += noise() * vol * math.exp(-t * 35)


def hat(buf, start, noise, vol=0.12):
    n = int(RATE * 0.03)
    prev = 0.0
    for i in range(n):
        if start + i >= len(buf):
            break
        x = noise()
        hp = x - prev  # rumore "più acuto"
        prev = x
        buf[start + i] += hp * vol * math.exp(-(i / RATE) * 120)


# ------------------------------------------------------------------ composizione

SCALES = {"minor": [0, 2, 3, 5, 7, 8, 10], "major": [0, 2, 4, 5, 7, 9, 11], "dorian": [0, 2, 3, 5, 7, 9, 10]}


def chord_tones(root: int, quality: str) -> list[int]:
    third = 3 if quality == "m" else 4
    return [root, root + third, root + 7]


def compose(seed: int, key: str, scale: str, progression: list[tuple[str, str]], bpm: int, bars_per_chord: int,
            loops: int, lead_octave: int, drums: str, lead_duty: float, harm: bool, density: float) -> list[float]:
    rnd = random.Random(seed)
    beat = 60 / bpm
    step = beat / 2  # crome
    steps_per_bar = 8
    total_bars = len(progression) * bars_per_chord * loops
    n = int(total_bars * steps_per_bar * step * RATE) + RATE // 2
    buf = [0.0] * n
    noise = Noise(seed)
    tonic = midi(key + "3")
    sc = SCALES[scale]
    scale_notes = [tonic + 12 * o + s for o in range(-1, 4) for s in sc]
    # Motivo ripetuto: la melodia di ogni accordo riusa un ritmo fisso, con
    # piccole variazioni al secondo giro (forma A A').
    rhythm = [1 if rnd.random() < density else 0 for _ in range(steps_per_bar * bars_per_chord)]
    rhythm[0] = 1
    lead_prev = tonic + 12 * (lead_octave - 3)
    bar = 0
    for loop in range(loops):
        for ci, (root_name, quality) in enumerate(progression):
            root = midi(root_name + "2")
            tones = chord_tones(root, quality)
            for b in range(bars_per_chord):
                bar_start = int(bar * steps_per_bar * step * RATE)
                # Basso: fondamentale e quinta in crome alternate.
                for s in range(steps_per_bar):
                    m = tones[0] if s % 4 != 2 else tones[2]
                    if s % 2 == 0 or drums == "battle":
                        mix_note(buf, bar_start + int(s * step * RATE), int(step * RATE * 0.9), freq(m), 0.16, "tri")
                # Armonia: accordo arpeggiato sopra.
                if harm:
                    for s in range(steps_per_bar):
                        m = tones[s % 3] + 12
                        mix_note(buf, bar_start + int(s * step * RATE), int(step * RATE * 0.8), freq(m), 0.045, "sq",
                                 duty=0.5, release=0.04)
                # Percussioni.
                for s in range(steps_per_bar):
                    pos = bar_start + int(s * step * RATE)
                    if drums == "battle":
                        if s in (0, 3, 4):
                            kick(buf, pos, 0.42)
                        if s in (2, 6):
                            snare(buf, pos, noise, 0.22)
                        hat(buf, pos, noise, 0.08)
                    elif drums == "soft":
                        if s == 0:
                            kick(buf, pos, 0.25)
                        if s in (2, 6):
                            hat(buf, pos, noise, 0.06)
                    elif drums == "march":
                        if s in (0, 4):
                            kick(buf, pos, 0.35)
                        if s in (2, 6, 7):
                            snare(buf, pos, noise, 0.14)
                # Melodia.
                for s in range(steps_per_bar):
                    k = b * steps_per_bar + s
                    if not rhythm[k] or (loop % 2 == 1 and k % 7 == 6 and rnd.random() < 0.5):
                        continue
                    strong = s % 4 == 0
                    if strong:
                        cands = [t + 12 * o for t in tones for o in range(lead_octave - 2, lead_octave)]
                    else:
                        cands = scale_notes
                    cands = [c for c in cands if abs(c - lead_prev) <= 7 and 55 <= c <= 86] or [lead_prev]
                    note = min(cands, key=lambda c: (abs(c - lead_prev) - rnd.random() * 4))
                    if rnd.random() < 0.25 and not strong:
                        note = lead_prev + rnd.choice([-2, -1, 1, 2])
                    lead_prev = note
                    length = 1
                    while k + length < len(rhythm) and not rhythm[k + length] and length < 3:
                        length += 1
                    mix_note(buf, bar_start + int(s * step * RATE), int(step * RATE * length * 0.95), freq(note),
                             0.11, "sq", duty=lead_duty, vibrato=0.0022 if length > 1 else 0.0)
                bar += 1
    return buf


def jingle(notes: list[tuple[str, float]], bpm: int, duty: float = 0.25, bass: list[tuple[str, float]] | None = None) -> list[float]:
    beat = 60 / bpm
    total = sum(d for _, d in notes) * beat + 0.6
    buf = [0.0] * int(total * RATE)
    t = 0.0
    for name, d in notes:
        if name != "-":
            mix_note(buf, int(t * RATE), int(d * beat * RATE * 0.95), freq(midi(name)), 0.16, "sq", duty=duty,
                     vibrato=0.0022, release=0.12)
        t += d * beat
    t = 0.0
    for name, d in bass or []:
        if name != "-":
            mix_note(buf, int(t * RATE), int(d * beat * RATE * 0.95), freq(midi(name)), 0.2, "tri")
        t += d * beat
    return buf


# ------------------------------------------------------------------ effetti

def sfx(kind: str) -> list[float]:
    noise = Noise(hash(kind) & 0xFFFF)
    if kind == "hit":
        buf = [0.0] * int(RATE * 0.18)
        kick(buf, 0, 0.6)
        snare(buf, 0, noise, 0.35)
        return buf
    if kind == "slash":
        n = int(RATE * 0.16)
        buf = [0.0] * n
        prev = 0.0
        for i in range(n):
            t = i / RATE
            x = noise()
            k = 0.3 + 0.6 * (1 - t / 0.16)
            prev = prev * (1 - k) + x * k
            buf[i] = (x - prev) * 0.5 * math.exp(-t * 14)
        return buf
    if kind in ("arrow", "miss"):
        n = int(RATE * 0.2)
        buf = [0.0] * n
        for i in range(n):
            t = i / RATE
            f = 1400 - 3000 * t if kind == "arrow" else 600 - 1500 * t
            buf[i] = math.sin(2 * math.pi * f * t) * 0.18 * math.exp(-t * 8) + noise() * 0.05 * math.exp(-t * 20)
        return buf
    if kind == "fire":
        n = int(RATE * 0.45)
        buf = [0.0] * n
        for i in range(n):
            t = i / RATE
            crackle = noise() if (i // 40) % 3 == 0 else noise() * 0.3
            buf[i] = crackle * 0.3 * math.sin(math.pi * min(1.0, t / 0.45))
        return buf
    if kind == "ice":
        buf = [0.0] * int(RATE * 0.5)
        for j, m in enumerate([84, 88, 91, 96]):
            mix_note(buf, int(j * 0.06 * RATE), int(0.25 * RATE), freq(m), 0.09, "sq", duty=0.125, release=0.15)
        return buf
    if kind == "thunder":
        n = int(RATE * 0.7)
        buf = [0.0] * n
        lp = 0.0
        for i in range(n):
            t = i / RATE
            lp = lp * 0.97 + noise() * 0.03
            buf[i] = noise() * 0.45 * math.exp(-t * 18) + lp * 4 * math.exp(-t * 3)
        return buf
    if kind == "holy":
        buf = [0.0] * int(RATE * 0.7)
        for m in (72, 76, 79, 84):
            mix_note(buf, 0, int(0.6 * RATE), freq(m), 0.06, "sq", duty=0.5, release=0.4)
        return buf
    if kind in ("heal", "buff", "levelup"):
        seq = {"heal": [72, 76, 79, 84], "buff": [67, 74], "levelup": [72, 76, 79, 84, 88]}[kind]
        buf = [0.0] * int(RATE * (0.09 * len(seq) + 0.3))
        for j, m in enumerate(seq):
            mix_note(buf, int(j * 0.09 * RATE), int(0.2 * RATE), freq(m), 0.12, "sq", duty=0.25, release=0.1)
        return buf
    if kind in ("debuff", "ko"):
        n = int(RATE * 0.4)
        buf = [0.0] * n
        for i in range(n):
            t = i / RATE
            f = (520 if kind == "debuff" else 330) * math.exp(-t * 3)
            buf[i] = square(f, t, 0.5) * 0.12 * (1 - t / 0.4)
        return buf
    if kind == "poison":
        buf = [0.0] * int(RATE * 0.45)
        for j in range(4):
            mix_note(buf, int(j * 0.1 * RATE), int(0.06 * RATE), freq(60 + j * 3), 0.1, "sq", duty=0.5)
        return buf
    if kind == "drain":
        n = int(RATE * 0.4)
        buf = [0.0] * n
        for i in range(n):
            t = i / RATE
            buf[i] = math.sin(2 * math.pi * (200 + 600 * t) * t) * 0.2 * (1 - t / 0.4)
        return buf
    if kind in ("click", "step"):
        n = int(RATE * (0.04 if kind == "click" else 0.03))
        buf = [0.0] * n
        for i in range(n):
            t = i / RATE
            buf[i] = (square(1200 if kind == "click" else 160, t, 0.5) * 0.1 + noise() * (0.0 if kind == "click" else 0.08)) * (1 - t / (n / RATE))
        return buf
    raise ValueError(kind)


# ------------------------------------------------------------------ uscita

def save(name: str, buf: list[float], peak: float = 0.85):
    m = max(1e-6, max(abs(x) for x in buf))
    k = peak / m if m > peak else 1.0
    OUT.mkdir(parents=True, exist_ok=True)
    with wave.open(str(OUT / f"{name}.wav"), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(b"".join(struct.pack("<h", int(max(-1, min(1, x * k)) * 32767)) for x in buf))
    print(f"{name}.wav  {len(buf) / RATE:.1f}s")


def main():
    save("music_menu", compose(11, "C", "major", [("C", ""), ("A", "m"), ("F", ""), ("G", "")], 92, 2, 2, 5, "soft", 0.5, True, 0.45))
    save("music_battle", compose(23, "A", "minor", [("A", "m"), ("F", ""), ("C", ""), ("G", ""), ("A", "m"), ("D", "m"), ("E", ""), ("E", "")], 138, 1, 3, 5, "battle", 0.25, True, 0.7))
    save("music_world", compose(37, "D", "dorian", [("D", "m"), ("G", ""), ("D", "m"), ("C", ""), ("F", ""), ("C", ""), ("G", ""), ("A", "m")], 112, 1, 3, 5, "march", 0.25, True, 0.55))
    save("jingle_victory", jingle([("C5", 0.5), ("E5", 0.5), ("G5", 0.5), ("C6", 1.5), ("G5", 0.5), ("C6", 2)], 140,
                                  bass=[("C3", 1.5), ("G2", 1.5), ("C3", 3)]))
    save("jingle_defeat", jingle([("A4", 1), ("G4", 1), ("F4", 1), ("E4", 2.5)], 90, duty=0.5,
                                 bass=[("A2", 2), ("D2", 1), ("E2", 2.5)]))
    for k in ["hit", "slash", "arrow", "miss", "fire", "ice", "thunder", "holy", "heal", "buff", "debuff", "poison",
              "drain", "levelup", "ko", "click", "step"]:
        save(f"sfx_{k}", sfx(k))


if __name__ == "__main__":
    main()

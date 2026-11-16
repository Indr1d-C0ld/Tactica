"""Motore di sintesi: FM a operatori in stile OPL3 (Sound Blaster) e strumenti
"orchestrali" con coro e riverbero in stile modulo MIDI anni '90 (MT-32).

Tutto è deterministico (rumore con seed fisso) e calcolato con numpy.
"""

from __future__ import annotations

import math
from dataclasses import dataclass, field

import numpy as np

SR = 32000

NOTE = {"C": 0, "C#": 1, "Db": 1, "D": 2, "D#": 3, "Eb": 3, "E": 4, "F": 5, "F#": 6, "Gb": 6,
        "G": 7, "G#": 8, "Ab": 8, "A": 9, "A#": 10, "Bb": 10, "B": 11}


def midi(name: str) -> int:
    n, o = name[:-1], int(name[-1])
    return 12 * (o + 1) + NOTE[n]


def hz(m: float) -> float:
    return 440.0 * 2 ** ((m - 69) / 12)


# ------------------------------------------------------------------ forme d'onda OPL

def wave(kind: str, ph: np.ndarray) -> np.ndarray:
    s = np.sin(ph)
    if kind == "sine":
        return s
    if kind == "half":  # mezza sinusoide (OPL forma 1)
        return np.maximum(s, 0.0) * 2 - 0.6
    if kind == "abs":  # sinusoide raddrizzata (OPL forma 2)
        return np.abs(s) * 2 - 1.27
    if kind == "pulse":  # quarto di sinusoide (OPL forma 3)
        q = np.where((ph % (2 * np.pi)) < np.pi / 2, s, 0.0)
        return q * 2 - 0.6
    if kind == "square":  # OPL3 forma 6
        return np.sign(s)
    if kind == "saw":  # OPL3 forma 7 (dente di sega derivata)
        return 1 - 2 * ((ph / (2 * np.pi)) % 1.0)
    raise ValueError(kind)


def adsr(n: int, gate: int, a: float, d: float, s: float, r: float) -> np.ndarray:
    """Inviluppo: attacco, decadimento, sostegno (livello), rilascio (in secondi)."""
    t = np.arange(n) / SR
    g = gate / SR
    e = np.where(t < a, t / max(a, 1e-4), s + (1 - s) * np.exp(-(t - a) / max(d, 1e-4)))
    rel_start = np.interp(g, t, e) if n else 0.0
    e = np.where(t < g, e, rel_start * np.exp(-(t - g) / max(r, 1e-4)))
    return e


# ------------------------------------------------------------------ patch FM

@dataclass
class Patch:
    """Due operatori (modulante -> portante), più un secondo portante opzionale
    (come i quattro operatori OPL3) e qualche abbellimento."""
    ratio: float = 1.0
    index: float = 1.0          # indice di modulazione iniziale
    index_end: float = 0.3      # indice finale (brillantezza che si spegne)
    index_decay: float = 0.3
    feedback: float = 0.0
    car_wave: str = "sine"
    mod_wave: str = "sine"
    a: float = 0.005
    d: float = 0.3
    s: float = 0.6
    r: float = 0.2
    vibrato: float = 0.0        # profondità in semitoni
    vib_rate: float = 5.5
    vib_delay: float = 0.25
    detune: float = 0.0         # secondo portante stonato (coro, in centesimi)
    ratio2: float | None = None  # secondo portante a un'altra ottava/rapporto
    mix2: float = 0.5
    noise: float = 0.0          # soffio (flauti)
    lowpass: float = 0.0        # 0 = nessuno; altrimenti coefficiente 0..1
    gain: float = 1.0


def _lowpass(x: np.ndarray, k: float) -> np.ndarray:
    # Filtro a un polo in forma vettoriale (approssimato con media mobile esponenziale).
    if k <= 0:
        return x
    a = 1 - k
    # Risposta all'impulso esponenziale troncata, applicata con convoluzione.
    taps = int(min(400, max(4, 6 / max(1e-3, -math.log(max(a, 1e-6))))))
    h = (1 - a) * a ** np.arange(taps)
    return np.convolve(x, h / h.sum(), mode="full")[: len(x)]


def fm_note(p: Patch, m: float, dur: float, vel: float = 1.0, seed: int = 0) -> np.ndarray:
    gate = int(dur * SR)
    n = gate + int(p.r * 5 * SR) + 1
    t = np.arange(n) / SR
    f = hz(m)
    vib = 0.0
    if p.vibrato:
        ramp = np.clip((t - p.vib_delay) / 0.3, 0, 1)
        vib = p.vibrato * ramp * np.sin(2 * np.pi * p.vib_rate * t)
    fr = f * 2 ** (vib / 12) if p.vibrato else np.full(n, f)
    ph = 2 * np.pi * np.cumsum(fr) / SR
    idx = p.index_end + (p.index - p.index_end) * np.exp(-t / max(p.index_decay, 1e-3))
    mod = wave(p.mod_wave, ph * p.ratio)
    if p.feedback:
        # Feedback approssimato: il modulatore modula sé stesso una volta.
        mod = wave(p.mod_wave, ph * p.ratio + p.feedback * mod)
    out = wave(p.car_wave, ph + idx * mod)
    if p.detune:
        ph2 = ph * 2 ** (p.detune / 1200)
        out = 0.6 * out + 0.6 * wave(p.car_wave, ph2 + idx * wave(p.mod_wave, ph2 * p.ratio))
    if p.ratio2:
        out = (1 - p.mix2) * out + p.mix2 * wave(p.car_wave, ph * p.ratio2 + idx * 0.5 * mod)
    if p.noise:
        rng = np.random.default_rng(seed + int(m * 7))
        out = out + p.noise * _lowpass(rng.uniform(-1, 1, n), 0.7)
    out = _lowpass(out, p.lowpass)
    e = adsr(n, gate, p.a, p.d, p.s, p.r)
    return (out * e * vel * p.gain).astype(np.float32)


# ------------------------------------------------------------------ strumenti

INSTR: dict[str, Patch] = {
    # Archi "LA": attacco lento, coro, poca modulazione.
    "strings": Patch(ratio=1.0, index=1.2, index_end=0.9, index_decay=1.0, car_wave="sine", mod_wave="saw",
                     a=0.25, d=1.0, s=0.85, r=0.5, vibrato=0.08, detune=9, lowpass=0.55, gain=0.55),
    "strings_short": Patch(ratio=1.0, index=1.6, index_end=0.7, index_decay=0.12, mod_wave="saw",
                           a=0.01, d=0.15, s=0.3, r=0.08, detune=7, lowpass=0.4, gain=0.55),
    "brass": Patch(ratio=1.0, index=0.6, index_end=2.6, index_decay=0.08, feedback=0.4, a=0.04, d=0.4, s=0.8,
                   r=0.15, vibrato=0.12, vib_delay=0.35, detune=4, lowpass=0.35, gain=0.6),
    "horn": Patch(ratio=1.0, index=1.4, index_end=1.0, index_decay=0.2, a=0.07, d=0.5, s=0.8, r=0.25,
                  vibrato=0.08, lowpass=0.6, gain=0.6),
    "flute": Patch(ratio=1.0, index=0.25, index_end=0.15, a=0.05, d=0.3, s=0.85, r=0.12, vibrato=0.18,
                   vib_delay=0.18, noise=0.05, lowpass=0.2, gain=0.5),
    "harp": Patch(ratio=2.0, index=2.2, index_end=0.2, index_decay=0.12, a=0.002, d=0.6, s=0.0, r=0.6,
                  gain=0.55),
    "epiano": Patch(ratio=14.0, index=1.4, index_end=0.0, index_decay=0.05, ratio2=1.0, mix2=0.75,
                    a=0.002, d=1.2, s=0.15, r=0.4, gain=0.55),
    "bell": Patch(ratio=3.5, index=3.0, index_end=0.3, index_decay=0.6, a=0.001, d=1.6, s=0.0, r=1.2,
                  gain=0.35),
    "glock": Patch(ratio=7.0, index=1.8, index_end=0.1, index_decay=0.2, a=0.001, d=0.8, s=0.0, r=0.5,
                   gain=0.35),
    "choir": Patch(ratio=2.0, index=0.5, index_end=0.4, mod_wave="half", a=0.35, d=1.0, s=0.9, r=0.6,
                   vibrato=0.1, detune=12, lowpass=0.5, gain=0.45),
    "organ": Patch(ratio=2.0, index=0.4, index_end=0.4, ratio2=3.0, mix2=0.3, a=0.01, d=0.5, s=0.95, r=0.1,
                   gain=0.45),
    "bass": Patch(ratio=1.0, index=2.2, index_end=0.6, index_decay=0.15, a=0.003, d=0.35, s=0.55, r=0.08,
                  lowpass=0.3, gain=0.8),
    "slapbass": Patch(ratio=1.0, index=4.0, index_end=0.8, index_decay=0.06, feedback=0.6, a=0.002, d=0.2,
                      s=0.45, r=0.06, lowpass=0.2, gain=0.8),
    "pizz": Patch(ratio=1.0, index=1.5, index_end=0.2, index_decay=0.05, a=0.002, d=0.18, s=0.0, r=0.1,
                  gain=0.6),
    "lead": Patch(ratio=1.0, index=2.0, index_end=1.4, index_decay=0.2, feedback=0.8, car_wave="sine",
                  mod_wave="sine", a=0.01, d=0.3, s=0.8, r=0.1, vibrato=0.15, vib_delay=0.2, lowpass=0.25,
                  gain=0.45),
    "synth_lead": Patch(ratio=1.0, index=2.6, index_end=1.8, feedback=1.2, car_wave="half", a=0.005, d=0.2,
                        s=0.75, r=0.08, vibrato=0.12, vib_delay=0.15, lowpass=0.3, gain=0.4),
    "timpani": Patch(ratio=1.5, index=1.8, index_end=0.2, index_decay=0.08, a=0.002, d=0.6, s=0.0, r=0.5,
                     lowpass=0.5, gain=0.9),
}


def instrument(name: str, m: float, dur: float, vel: float = 1.0) -> np.ndarray:
    return fm_note(INSTR[name], m, dur, vel, seed=hash(name) % 1000)


# ------------------------------------------------------------------ percussioni

_RNG = np.random.default_rng(1234)


def noise(n: int, seed: int = 0) -> np.ndarray:
    return np.random.default_rng(seed).uniform(-1, 1, n).astype(np.float32)


def highpass(x: np.ndarray, k: float = 0.6) -> np.ndarray:
    return x - _lowpass(x, k)


def drum(kind: str, vel: float = 1.0, seed: int = 0) -> np.ndarray:
    if kind == "kick":
        n = int(0.35 * SR)
        t = np.arange(n) / SR
        f = 45 + 120 * np.exp(-t / 0.03)
        ph = 2 * np.pi * np.cumsum(f) / SR
        x = np.sin(ph + 0.8 * np.sin(ph)) * np.exp(-t / 0.12)
        x[: int(0.004 * SR)] += noise(int(0.004 * SR), seed) * 0.4
    elif kind == "snare":
        n = int(0.25 * SR)
        t = np.arange(n) / SR
        tone = np.sin(2 * np.pi * 190 * t) * np.exp(-t / 0.05)
        x = 0.5 * tone + 0.8 * highpass(noise(n, seed + 1), 0.5) * np.exp(-t / 0.09)
    elif kind == "hat":
        n = int(0.08 * SR)
        t = np.arange(n) / SR
        x = 0.5 * highpass(noise(n, seed + 2), 0.85) * np.exp(-t / 0.02)
    elif kind == "openhat":
        n = int(0.3 * SR)
        t = np.arange(n) / SR
        x = 0.4 * highpass(noise(n, seed + 3), 0.85) * np.exp(-t / 0.12)
    elif kind == "crash":
        n = int(1.6 * SR)
        t = np.arange(n) / SR
        x = 0.5 * highpass(noise(n, seed + 4), 0.75) * np.exp(-t / 0.6)
    elif kind == "tom":
        n = int(0.35 * SR)
        t = np.arange(n) / SR
        f = 90 + 60 * np.exp(-t / 0.05)
        x = np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-t / 0.15)
    elif kind == "tambourine":
        n = int(0.15 * SR)
        t = np.arange(n) / SR
        x = 0.35 * highpass(noise(n, seed + 5), 0.9) * np.exp(-t / 0.05) * (1 + 0.5 * np.sin(2 * np.pi * 60 * t))
    else:
        raise ValueError(kind)
    return (x * vel).astype(np.float32)


# ------------------------------------------------------------------ mix

def add(buf: np.ndarray, x: np.ndarray, at: int, pan: float = 0.0, wrap: bool = True) -> None:
    """Somma `x` (mono) in `buf` (stereo) dal campione `at`; con `wrap` la coda
    rientra all'inizio (loop senza giunture)."""
    n = buf.shape[0]
    l = math.cos((pan + 1) * math.pi / 4)
    r = math.sin((pan + 1) * math.pi / 4)
    end = at + len(x)
    if end <= n:
        buf[at:end, 0] += x * l
        buf[at:end, 1] += x * r
    else:
        k = n - at
        buf[at:, 0] += x[:k] * l
        buf[at:, 1] += x[:k] * r
        if wrap:
            rest = x[k:]
            while len(rest):
                m = min(len(rest), n)
                buf[:m, 0] += rest[:m] * l
                buf[:m, 1] += rest[:m] * r
                rest = rest[m:]


def reverb(buf: np.ndarray, seconds: float = 2.2, wet: float = 0.25, circular: bool = True,
           seed: int = 7) -> np.ndarray:
    """Riverbero a convoluzione con una risposta sintetica (rumore che decade,
    diverso per i due canali). `circular`: la coda rientra all'inizio."""
    n = buf.shape[0]
    m = int(seconds * SR)
    t = np.arange(m) / SR
    out = np.empty_like(buf)
    for ch in range(2):
        ir = np.random.default_rng(seed + ch).normal(0, 1, m) * np.exp(-t * 6.9 / seconds)
        ir = _lowpass(ir, 0.35)
        ir[: int(0.012 * SR)] = 0  # pre-ritardo
        ir /= np.sqrt((ir ** 2).sum()) + 1e-9
        size = n if circular else n + m
        size = 1 << (size - 1).bit_length()
        if circular:
            h = np.zeros(n)
            h[: min(m, n)] = ir[: min(m, n)]
            y = np.fft.irfft(np.fft.rfft(buf[:, ch]) * np.fft.rfft(h), n)
        else:
            y = np.fft.irfft(np.fft.rfft(buf[:, ch], size) * np.fft.rfft(ir, size), size)[:n]
        out[:, ch] = buf[:, ch] + wet * y
    return out


def normalize(buf: np.ndarray, peak: float = 0.89) -> np.ndarray:
    # Compressione morbida e normalizzazione.
    m = np.max(np.abs(buf)) + 1e-9
    x = buf / m
    x = np.tanh(x * 1.3) / np.tanh(1.3)
    return (x * peak).astype(np.float32)


# ------------------------------------------------------------------ sequenze

@dataclass
class Song:
    bpm: float
    beats_per_bar: int
    bars: int
    tracks: list = field(default_factory=list)  # (strumento, [(battito, durata, nota, vel)], pan)
    drums: list = field(default_factory=list)   # (battito, tipo, vel)
    reverb: float = 0.25
    reverb_len: float = 2.2

    @property
    def beat(self) -> float:
        return 60.0 / self.bpm

    def length(self) -> int:
        return int(self.bars * self.beats_per_bar * self.beat * SR)

    def render(self) -> np.ndarray:
        n = self.length()
        buf = np.zeros((n, 2), dtype=np.float32)
        cache: dict = {}
        for name, notes, pan in self.tracks:
            for b, dur, m, vel in notes:
                key = (name, round(m, 2), round(dur, 3))
                if key not in cache:
                    cache[key] = instrument(name, m, dur * self.beat)
                add(buf, cache[key] * vel, int(b * self.beat * SR), pan)
        for b, kind, vel in self.drums:
            key = ("drum", kind)
            if key not in cache:
                cache[key] = drum(kind)
            pan = {"hat": 0.3, "openhat": 0.3, "tambourine": -0.35, "tom": -0.2}.get(kind, 0.0)
            add(buf, cache[key] * vel, int(b * self.beat * SR), pan)
        return normalize(reverb(buf, self.reverb_len, self.reverb))


# ------------------------------------------------------------------ armonia

QUALITY = {"": [0, 4, 7], "m": [0, 3, 7], "7": [0, 4, 7, 10], "m7": [0, 3, 7, 10], "maj7": [0, 4, 7, 11],
           "sus4": [0, 5, 7], "dim": [0, 3, 6], "add9": [0, 4, 7, 14], "m9": [0, 3, 7, 10, 14]}


def chord(sym: str, octave: int = 3) -> list[int]:
    """'Dm7' -> note MIDI dalla fondamentale all'ottava indicata."""
    root = sym[:2] if len(sym) > 1 and sym[1] in "#b" else sym[:1]
    q = sym[len(root):]
    base = 12 * (octave + 1) + NOTE[root]
    return [base + i for i in QUALITY[q]]


def parse_line(text: str, start: float = 0.0) -> list[tuple[float, float, int, float]]:
    """Melodia in notazione compatta: 'E5:1 D5:0.5 r:0.5 C5:2' (r = pausa; '!' accento)."""
    out = []
    b = start
    for tok in text.split():
        if tok == "|":
            continue
        name, dur = tok.split(":")
        d = float(dur)
        if name != "r":
            vel = 1.0
            if name.endswith("!"):
                name, vel = name[:-1], 1.25
            out.append((b, d * 0.95, midi(name), vel))
        b += d
    return out

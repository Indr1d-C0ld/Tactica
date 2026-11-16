"""Musiche originali di Tactica, composte in codice per il motore fm.py.

Stile: moduli MIDI anni '90 (archi, ottoni, coro, timpani con riverbero) e FM
della Sound Blaster (lead e bassi a operatori). Le musiche sono in loop senza
giunture (coda del riverbero riavvolta all'inizio); le fanfare no.

  python3 tools/audio/music.py   # scrive client/assets/audio/music_*.ogg e jingle_*.ogg
"""

from __future__ import annotations

import subprocess
import tempfile
import wave
from pathlib import Path

import numpy as np

import fm
from fm import SR, Song, chord, midi, parse_line

OUT = Path(__file__).resolve().parents[2] / "client" / "assets" / "audio"


# ------------------------------------------------------------------ aiuti di arrangiamento

def pad(song: Song, prog: list[str], instr: str, octave: int, start_bar: int = 0, vel: float = 0.5,
        pan: float = 0.0, top: bool = False):
    bpb = song.beats_per_bar
    notes = []
    for i, c in enumerate(prog):
        ns = chord(c, octave)
        if top:
            ns = ns + [ns[0] + 12]
        for m in ns:
            notes.append(((start_bar + i) * bpb, bpb, m, vel))
    song.tracks.append((instr, notes, pan))


def arp(song: Song, prog: list[str], instr: str, octave: int, pattern: list[int], step: float,
        start_bar: int = 0, vel: float = 0.45, pan: float = 0.0, swing: float = 0.0):
    """Arpeggio: `pattern` indici nelle note dell'accordo (anche oltre: ottava sopra)."""
    bpb = song.beats_per_bar
    notes = []
    for i, c in enumerate(prog):
        ns = chord(c, octave)
        k = 0
        b = 0.0
        while b < bpb - 1e-6:
            j = pattern[k % len(pattern)]
            m = ns[j % len(ns)] + 12 * (j // len(ns))
            sw = swing if (k % 2 == 1) else 0.0
            notes.append(((start_bar + i) * bpb + b + sw, step * 0.9, m, vel))
            b += step
            k += 1
        song.tracks.append((instr, notes, pan)) if False else None
    song.tracks.append((instr, notes, pan))


def bassline(song: Song, prog: list[str], instr: str, octave: int, rhythm: list[tuple[float, float, int]],
             start_bar: int = 0, vel: float = 0.8):
    """`rhythm`: (battito, durata, intervallo in semitoni dalla fondamentale)."""
    bpb = song.beats_per_bar
    notes = []
    for i, c in enumerate(prog):
        root = chord(c, octave)[0]
        for b, d, iv in rhythm:
            notes.append(((start_bar + i) * bpb + b, d, root + iv, vel))
    song.tracks.append((instr, notes, 0.0))


def melody(song: Song, instr: str, text: str, start_bar: int = 0, vel: float = 0.9, pan: float = 0.0,
           octave_shift: int = 0):
    notes = [(b, d, m + 12 * octave_shift, v * vel) for b, d, m, v in parse_line(text, start_bar * song.beats_per_bar)]
    song.tracks.append((instr, notes, pan))


def drums(song: Song, pattern: dict[str, list[float]], bars: range, vel: dict[str, float] | None = None,
          swing: float = 0.0):
    vel = vel or {}
    for bar in bars:
        for kind, beats in pattern.items():
            for b in beats:
                sw = swing if (b % 1.0) == 0.5 else 0.0
                song.drums.append((bar * song.beats_per_bar + b + sw, kind, vel.get(kind, 0.8)))


def fill(song: Song, bar: int):
    """Rullata di tom e piatto per chiudere una sezione."""
    bpb = song.beats_per_bar
    for k, b in enumerate([2, 2.5, 3, 3.5]):
        song.drums.append((bar * bpb + b, "tom", 0.6 + 0.1 * k))
    song.drums.append(((bar + 1) * bpb % (song.bars * bpb), "crash", 0.6))


# ------------------------------------------------------------------ brani

def menu() -> Song:
    """«Le Terre di Velmara»: tema eroico e pastorale, re maggiore."""
    s = Song(bpm=92, beats_per_bar=4, bars=32, reverb=0.32, reverb_len=2.6)
    A = ["D", "Bm", "G", "A", "D", "F#m", "G", "A"]
    B = ["Bm", "G", "D", "A", "Em", "G", "A", "A"]
    form = A + A + B + A
    pad(s, form, "strings", 3, vel=0.45)
    arp(s, form, "harp", 4, [0, 1, 2, 3, 2, 1, 0, 2], 0.5, vel=0.32, pan=-0.35)
    bassline(s, form, "pizz", 2, [(0, 1, 0), (2, 1, 7), (3, 1, 12)], vel=0.55)
    mel_a = ("F#5:1.5 E5:0.5 D5:1 A4:1 | B4:1 D5:1 F#5:1.5 E5:0.5 | D5:1 B4:1 G4:1.5 A4:0.5 | A4:3 r:1 | "
             "F#5:1.5 G5:0.5 A5:1 F#5:1 | C#6:1.5 B5:0.5 A5:1 F#5:1 | G5:1 F#5:1 E5:1 D5:1 | E5:2 C#5:1 A4:1")
    mel_b = ("D5:2 C#5:1 B4:1 | B4:1.5 A4:0.5 G4:2 | A4:1 D5:1 F#5:2 | E5:3 r:1 | "
             "G5:1.5 F#5:0.5 E5:1 B4:1 | D5:1 E5:1 G5:2 | A5:2 G5:1 F#5:1 | E5:2 A4:2")
    melody(s, "flute", mel_a, 0, pan=0.2)
    melody(s, "flute", mel_a, 8, pan=0.2)
    melody(s, "strings", mel_a, 8, vel=0.35, octave_shift=-1, pan=-0.2)
    melody(s, "horn", mel_b, 16, pan=0.1)
    melody(s, "flute", mel_a, 24, pan=0.2)
    melody(s, "horn", mel_a, 24, vel=0.45, octave_shift=-1, pan=-0.15)
    for bar in range(16, 24):
        s.drums.append((bar * 4, "kick", 0.3))
        s.tracks.append(("timpani", [(bar * 4, 1, chord(B[bar - 16], 2)[0], 0.5)], 0.0))
    s.drums.append((16 * 4, "crash", 0.35))
    return s


def world() -> Song:
    """«In cammino»: 6/8 leggero, sol maggiore (un battito = una croma)."""
    s = Song(bpm=210, beats_per_bar=6, bars=32, reverb=0.28)
    A = ["G", "C", "D", "G", "Em", "C", "D", "D"]
    B = ["Am", "D", "G", "Em", "C", "D", "G", "D"]
    form = A + B + A + B
    bassline(s, form, "pizz", 2, [(0, 2, 0), (3, 2, 7)], vel=0.6)
    arp(s, form, "harp", 3, [0, 2, 3, 1, 2, 3], 1, vel=0.3, pan=-0.3)
    pad(s, form, "strings", 4, vel=0.22, pan=0.25)
    mel_a = ("D5:2 B4:1 G4:2 B4:1 | C5:3 E5:3 | D5:2 C5:1 B4:2 A4:1 | B4:3 G4:3 | "
             "E5:2 F#5:1 G5:2 E5:1 | C5:2 E5:1 G5:3 | F#5:2 E5:1 D5:2 C5:1 | A4:6")
    mel_b = ("E5:2 D5:1 C5:3 | D5:2 E5:1 F#5:3 | G5:2 F#5:1 E5:2 D5:1 | B4:6 | "
             "C5:2 D5:1 E5:3 | F#5:2 G5:1 A5:3 | G5:3 D5:3 | F#5:3 A4:3")
    melody(s, "flute", mel_a, 0, pan=0.15)
    melody(s, "flute", mel_b, 8, pan=0.15)
    melody(s, "epiano", mel_a, 16, vel=0.8, pan=0.1)
    melody(s, "flute", mel_a, 16, vel=0.5, octave_shift=1, pan=0.3)
    melody(s, "horn", mel_b, 24, vel=0.8, octave_shift=-1, pan=-0.1)
    drums(s, {"kick": [0], "tambourine": [3, 5]}, range(32), {"kick": 0.35, "tambourine": 0.4})
    return s


BATTLE_DRUMS = {"kick": [0, 1.5, 2], "snare": [1, 3], "hat": [0, 0.5, 1, 1.5, 2, 2.5, 3, 3.5]}


def battle() -> Song:
    """«Statuti in campo»: battaglia, la minore, ostinato e ottoni."""
    s = Song(bpm=144, beats_per_bar=4, bars=32, reverb=0.2, reverb_len=1.8)
    A = ["Am", "Am", "F", "G", "Am", "Am", "F", "E"]
    B = ["Dm", "Am", "F", "E", "Dm", "F", "G", "E"]
    form = A + A + B + A
    bassline(s, form, "bass", 2, [(b * 0.5, 0.45, 12 if b in (3, 7) else 0) for b in range(8)], vel=0.75)
    arp(s, form, "strings_short", 4, [0, 1, 2, 1], 0.5, vel=0.32, pan=-0.3)
    pad(s, form, "strings", 3, vel=0.25, pan=0.2)
    mel_a = ("A4:1 C5:0.5 E5:0.5 A5:1.5 G5:0.5 | E5:1 D5:0.5 C5:0.5 D5:1 E5:1 | F5:1.5 E5:0.5 D5:1 C5:1 | "
             "D5:1 B4:1 G4:1 B4:1 | A4:1 C5:0.5 E5:0.5 A5:1.5 B5:0.5 | C6:1 B5:0.5 A5:0.5 G5:1 E5:1 | "
             "F5:1 A5:1 G5:1 F5:1 | E5:2 G#5:1 B5:1")
    mel_b = ("D5:0.5 F5:0.5 A5:1 D6:1 C6:1 | C6:0.5 B5:0.5 A5:1 E5:2 | F5:0.5 G5:0.5 A5:1 C6:1 A5:1 | "
             "G#5:2 E5:2 | D6:1 C6:0.5 A5:0.5 F5:1 D5:1 | F5:1 A5:1 C6:2 | B5:1 D6:1 G5:1 B5:1 | "
             "E5:1 G#5:1 B5:1 E6:1")
    melody(s, "brass", mel_a, 0, pan=0.1)
    melody(s, "brass", mel_a, 8, pan=0.1)
    melody(s, "lead", mel_a, 8, vel=0.45, octave_shift=1, pan=0.35)
    melody(s, "synth_lead", mel_b, 16, pan=0.2)
    melody(s, "strings", mel_b, 16, vel=0.4, octave_shift=-1, pan=-0.2)
    melody(s, "brass", mel_a, 24, pan=0.1)
    melody(s, "strings", mel_a, 24, vel=0.35, octave_shift=1, pan=-0.25)
    drums(s, BATTLE_DRUMS, range(32), {"kick": 0.9, "snare": 0.7, "hat": 0.35})
    for bar in (7, 15, 23, 31):
        fill(s, bar)
    return s


def battle2() -> Song:
    """«Vessilli contro»: scontri fra giocatori e clan, mi minore, FM più marcata."""
    s = Song(bpm=156, beats_per_bar=4, bars=32, reverb=0.18, reverb_len=1.6)
    A = ["Em", "C", "D", "Em", "Em", "C", "D", "B7"]
    B = ["Am", "Em", "C", "D", "Am", "C", "B7", "B7"]
    form = A + A + B + A
    bassline(s, form, "slapbass", 2, [(0, 0.45, 0), (0.5, 0.45, 12), (1, 0.45, 0), (1.75, 0.25, 0),
                                       (2, 0.45, 7), (2.5, 0.45, 12), (3, 0.45, 10), (3.5, 0.45, 7)], vel=0.75)
    arp(s, form, "epiano", 4, [0, 2, 1, 2], 0.5, vel=0.28, pan=-0.35)
    pad(s, form, "organ", 3, vel=0.18, pan=0.3)
    mel_a = ("E5:0.5 G5:0.5 B5:1 A5:0.5 G5:0.5 F#5:1 | E5:1 G5:1 C6:1.5 B5:0.5 | A5:1 F#5:1 D5:1 F#5:1 | "
             "E5:3 r:1 | B5:0.5 A5:0.5 G5:1 F#5:0.5 G5:0.5 A5:1 | G5:1 E5:1 C5:1 E5:1 | "
             "F#5:1 A5:1 D6:1 C6:1 | B5:2 D#5:2")
    mel_b = ("A5:1.5 G5:0.5 E5:1 C5:1 | B4:1 E5:1 G5:2 | C6:1 B5:0.5 A5:0.5 G5:1 E5:1 | F#5:2 D5:2 | "
             "E6:1 D6:0.5 C6:0.5 A5:2 | G5:1 C6:1 E6:2 | D#6:1 C6:0.5 B5:0.5 A5:1 F#5:1 | B5:4")
    melody(s, "synth_lead", mel_a, 0, pan=0.15)
    melody(s, "synth_lead", mel_a, 8, pan=0.15)
    melody(s, "lead", mel_a, 8, vel=0.4, octave_shift=-1, pan=-0.3)
    melody(s, "brass", mel_b, 16, pan=0.1)
    melody(s, "synth_lead", mel_a, 24, pan=0.15)
    melody(s, "brass", mel_a, 24, vel=0.45, octave_shift=-1, pan=-0.2)
    drums(s, {"kick": [0, 0.75, 2, 2.5], "snare": [1, 3], "hat": [b * 0.5 for b in range(8)],
              "openhat": [3.5]}, range(32), {"kick": 0.95, "snare": 0.75, "hat": 0.3, "openhat": 0.3})
    for bar in (15, 31):
        fill(s, bar)
    return s


def boss() -> Song:
    """«Il campione»: re minore, coro, organo e timpani."""
    s = Song(bpm=112, beats_per_bar=4, bars=24, reverb=0.38, reverb_len=3.0)
    A = ["Dm", "Bb", "Gm", "A", "Dm", "Bb", "C", "A"]
    B = ["Gm", "Dm", "Bb", "A", "Gm", "Bb", "A", "A"]
    form = A + B + A
    pad(s, form, "choir", 3, vel=0.4)
    pad(s, form, "organ", 2, vel=0.2, pan=-0.2)
    bassline(s, form, "strings_short", 2, [(b * 0.5, 0.4, 0 if b % 4 else 12) for b in range(8)], vel=0.5)
    mel_a = ("D5:2 F5:1 A5:1 | Bb5:2 A5:1 F5:1 | G5:1 Bb5:1 D6:2 | C#6:3 A5:1 | D6:1.5 C6:0.5 A5:1 F5:1 | "
             "G5:1.5 F5:0.5 D5:2 | E5:1 G5:1 C6:1 E5:1 | E5:2 C#5:2")
    mel_b = ("G5:2 Bb5:2 | A5:2 F5:2 | F5:1 G5:1 Bb5:2 | A5:4 | D6:2 C6:1 Bb5:1 | Bb5:2 A5:1 G5:1 | "
             "A5:2 C#6:2 | E6:4")
    melody(s, "brass", mel_a, 0, pan=0.15)
    melody(s, "horn", mel_b, 8, pan=0.1)
    melody(s, "choir", mel_b, 8, vel=0.5, octave_shift=-1, pan=-0.1)
    melody(s, "brass", mel_a, 16, pan=0.15)
    melody(s, "strings", mel_a, 16, vel=0.45, octave_shift=-1, pan=-0.25)
    for bar in range(24):
        root = chord(form[bar], 2)[0]
        s.tracks.append(("timpani", [(bar * 4, 1, root, 0.7), (bar * 4 + 2.5, 0.5, root, 0.4),
                                     (bar * 4 + 3, 1, root + 7 - 12, 0.5)], 0.0))
    drums(s, {"kick": [0, 2], "snare": [3], "crash": []}, range(24), {"kick": 0.6, "snare": 0.35})
    for bar in (0, 8, 16):
        s.drums.append((bar * 4, "crash", 0.45))
    return s


def shop() -> Song:
    """«La bottega»: fa maggiore, swing, piano elettrico FM e basso."""
    s = Song(bpm=96, beats_per_bar=4, bars=32, reverb=0.22)
    A = ["Fmaj7", "Dm7", "Gm7", "C7", "Fmaj7", "Dm7", "Gm7", "C7"]
    B = ["Bbmaj7", "Am7", "Gm7", "C7", "Fmaj7", "D7", "Gm7", "C7"]
    form = A + B + A + B
    sw = 0.17
    # Basso camminante: fondamentale, terza, quinta, passaggio cromatico.
    bassline(s, form, "slapbass", 2, [(0, 0.9, 0), (1, 0.9, 4), (2, 0.9, 7), (3, 0.9, 6)], vel=0.6)
    for i, c in enumerate(form):
        ns = chord(c, 4)
        s.tracks.append(("epiano", [(i * 4 + 0.5 + sw, 0.4, m, 0.32) for m in ns[1:]] +
                         [(i * 4 + 2.5 + sw, 0.9, m, 0.28) for m in ns[1:]], -0.25))
    mel_a = ("A5:1 C6:0.5 A5:0.5 G5:1 F5:1 | F5:1 E5:0.5 D5:0.5 C5:2 | D5:1 F5:1 Bb5:1 A5:1 | "
             "G5:2 E5:1 C5:1 | A5:1 C6:0.5 A5:0.5 F6:1 E6:1 | D6:1 C6:1 A5:2 | "
             "Bb5:1 A5:0.5 G5:0.5 F5:1 D5:1 | E5:2 G5:2")
    mel_b = ("D6:1.5 C6:0.5 Bb5:1 A5:1 | C6:1.5 A5:0.5 G5:1 E5:1 | F5:1 G5:1 Bb5:1 D6:1 | "
             "C6:2 Bb5:1 G5:1 | A5:1 F5:1 C5:1 F5:1 | F#5:1 A5:1 C6:1 D6:1 | Bb5:1.5 A5:0.5 G5:1 E5:1 | F5:2 r:2")
    melody(s, "glock", mel_a, 0, pan=0.25)
    melody(s, "glock", mel_b, 8, pan=0.25)
    melody(s, "flute", mel_a, 16, vel=0.7, octave_shift=-1, pan=0.15)
    melody(s, "glock", mel_b, 24, pan=0.25)
    melody(s, "epiano", mel_b, 24, vel=0.5, octave_shift=-1, pan=0.0)
    drums(s, {"kick": [0, 2.5], "snare": [1, 3], "hat": [0, 0.5, 1, 1.5, 2, 2.5, 3, 3.5]}, range(32),
          {"kick": 0.4, "snare": 0.22, "hat": 0.2}, swing=sw)
    return s


def victory() -> Song:
    s = Song(bpm=120, beats_per_bar=4, bars=3, reverb=0.3)
    melody(s, "brass", "G4:0.33 G4:0.33 G4:0.33 C5:1 E5:0.5 G5:0.5 C6:2 r:1 | B5:0.5 C6:0.5 D6:1 C6:2", 0)
    melody(s, "brass", "E4:0.33 E4:0.33 E4:0.33 G4:1 C5:0.5 E5:0.5 G5:2 r:1 | G5:0.5 A5:0.5 B5:1 G5:2", 0,
           vel=0.6, pan=-0.3)
    pad(s, ["C", "C", "C"], "strings", 3, vel=0.4)
    for b in [0, 1, 1.33, 1.66, 2, 4, 8]:
        s.tracks.append(("timpani", [(b, 0.5, midi("C3"), 0.6)], 0.0))
    s.drums.append((2, "crash", 0.5))
    s.drums.append((8, "crash", 0.4))
    return s


def defeat() -> Song:
    s = Song(bpm=70, beats_per_bar=4, bars=3, reverb=0.4, reverb_len=3.0)
    melody(s, "strings", "E5:1 D5:1 C5:1 B4:1 | A4:2 G#4:2 | A4:4", 0, vel=0.9)
    pad(s, ["Am", "Dm", "Am"], "choir", 3, vel=0.4)
    s.tracks.append(("timpani", [(0, 2, midi("A2"), 0.5), (8, 3, midi("A2"), 0.6)], 0.0))
    return s


SONGS = {
    "music_menu": menu,
    "music_world": world,
    "music_battle": battle,
    "music_battle2": battle2,
    "music_boss": boss,
    "music_shop": shop,
}
JINGLES = {"jingle_victory": victory, "jingle_defeat": defeat}


def render_jingle(song: Song) -> np.ndarray:
    """Le fanfare non vanno in loop: coda di riverbero libera e dissolvenza."""
    n = song.length() + int(2.5 * SR)
    buf = np.zeros((n, 2), dtype=np.float32)
    for name, notes, pan in song.tracks:
        for b, dur, m, vel in notes:
            fm.add(buf, fm.instrument(name, m, dur * song.beat) * vel, int(b * song.beat * SR), pan, wrap=False)
    for b, kind, vel in song.drums:
        fm.add(buf, fm.drum(kind) * vel, int(b * song.beat * SR), 0.0, wrap=False)
    out = fm.reverb(buf, song.reverb_len, song.reverb, circular=False)
    fade = int(0.8 * SR)
    out[-fade:] *= np.linspace(1, 0, fade)[:, None]
    return fm.normalize(out, 0.85)


def write_ogg(path: Path, x: np.ndarray) -> None:
    with tempfile.NamedTemporaryFile(suffix=".wav") as tmp:
        with wave.open(tmp.name, "wb") as w:
            w.setnchannels(2)
            w.setsampwidth(2)
            w.setframerate(SR)
            w.writeframes((np.clip(x, -1, 1) * 32767).astype("<i2").tobytes())
        subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", tmp.name, "-c:a", "libvorbis",
                        "-q:a", "5", str(path)], check=True)


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    for name, fn in SONGS.items():
        x = fn().render()
        write_ogg(OUT / f"{name}.ogg", x)
        print(f"{name}.ogg  {len(x) / SR:.1f} s")
    for name, fn in JINGLES.items():
        x = render_jingle(fn())
        write_ogg(OUT / f"{name}.ogg", x)
        print(f"{name}.ogg  {len(x) / SR:.1f} s")


if __name__ == "__main__":
    main()

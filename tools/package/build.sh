#!/usr/bin/env bash
# Crea i pacchetti del client per i giocatori:
#   dist/tactica-linux.tar.gz   (Linux x86_64)
#   dist/tactica-windows.zip    (Windows 10/11 x86_64)
#
#   tools/package/build.sh --server https://gioco.esempio.it          # entrambi
#   tools/package/build.sh --server https://gioco.esempio.it linux    # solo Linux
#
# Servono i modelli di esportazione di Godot 4.7.2 in
# ~/.local/share/godot/export_templates/4.7.2.stable/ e, per Windows, il
# compilatore llvm-mingw in ~/Android/llvm-mingw (vedi docs/CLIENT.md).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SERVER="http://127.0.0.1:7777"
GODOT_BIN="${GODOT_BIN:-$HOME/Android/godot/Godot_v4.7.2-stable_linux.x86_64}"
MINGW="${MINGW:-$HOME/Android/llvm-mingw}"
TEMPLATES="$HOME/.local/share/godot/export_templates/4.7.2.stable"
WIN_TARGET=x86_64-pc-windows-gnullvm
TARGETS=()

while [ $# -gt 0 ]; do
    case "$1" in
        --server) SERVER="${2:?manca URL}"; shift 2 ;;
        linux|windows) TARGETS+=("$1"); shift ;;
        -h|--help) sed -n '2,11p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "argomento sconosciuto: $1" >&2; exit 2 ;;
    esac
done
[ ${#TARGETS[@]} -gt 0 ] || TARGETS=(linux windows)
[[ "$SERVER" =~ ^https?://[A-Za-z0-9.:-]+$ ]] || { echo "URL del server non valido: $SERVER" >&2; exit 1; }
[ -x "$GODOT_BIN" ] || { echo "motore Godot non trovato: $GODOT_BIN (imposta GODOT_BIN)" >&2; exit 1; }
[ -f "$TEMPLATES/version.txt" ] || { echo "modelli di esportazione mancanti in $TEMPLATES" >&2; exit 1; }

cd "$ROOT"
echo "==> Grafica e import delle risorse"
python3 tools/pixelart/build.py >/dev/null
"$GODOT_BIN" --headless --path client --import >/dev/null 2>&1 || true

readme() {
    local run="$1"
    cat <<EOF
Tactica

Avvio:    $run
Server:   $SERVER
          (si cambia dalla schermata di accesso, o per sempre in server.txt)
Offline:  $run -- --offline   (schermaglia locale, senza server)

Requisiti: scheda video con OpenGL 3.3.
Il motore e' Godot Engine (licenza MIT, https://godotengine.org/license).
EOF
}

export_preset() {
    local preset="$1" out="$2" exe="$3"
    rm -rf "$out"
    mkdir -p "$out"
    "$GODOT_BIN" --headless --path client --export-release "$preset" "$out/$exe" >/dev/null 2>&1
    [ -s "$out/$exe" ] && [ -s "$out/${exe%.*}.pck" ] || { echo "esportazione $preset fallita" >&2; exit 1; }
    printf '%s\n' "$SERVER" >"$out/server.txt"
}

for t in "${TARGETS[@]}"; do
    case "$t" in
    linux)
        echo "==> Linux: estensione Rust"
        cargo build --release -p tactica-godot
        echo "==> Linux: esportazione"
        OUT="$ROOT/dist/tactica-linux"
        export_preset Linux "$OUT" Tactica.x86_64
        strip "$OUT/libtactica_godot.so"
        chmod 755 "$OUT/Tactica.x86_64"
        readme "./Tactica.x86_64" >"$OUT/LEGGIMI.txt"
        tar -C "$ROOT/dist" -czf "$ROOT/dist/tactica-linux.tar.gz" tactica-linux
        du -h "$ROOT/dist/tactica-linux.tar.gz"
        ;;
    windows)
        [ -x "$MINGW/bin/x86_64-w64-mingw32-clang" ] || { echo "llvm-mingw non trovato in $MINGW" >&2; exit 1; }
        echo "==> Windows: estensione Rust"
        PATH="$MINGW/bin:$PATH" \
            CARGO_TARGET_X86_64_PC_WINDOWS_GNULLVM_LINKER=x86_64-w64-mingw32-clang \
            cargo build --release -p tactica-godot --target "$WIN_TARGET"
        echo "==> Windows: esportazione"
        OUT="$ROOT/dist/tactica-windows"
        export_preset Windows "$OUT" Tactica.exe
        "$MINGW/bin/llvm-strip" "$OUT/tactica_godot.dll"
        readme "Tactica.exe" | sed 's/$/\r/' >"$OUT/LEGGIMI.txt"
        rm -f "$ROOT/dist/tactica-windows.zip"
        python3 - "$ROOT/dist" <<'PY'
import sys, zipfile, pathlib
d = pathlib.Path(sys.argv[1])
with zipfile.ZipFile(d / "tactica-windows.zip", "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
    for f in sorted((d / "tactica-windows").iterdir()):
        z.write(f, f"tactica-windows/{f.name}")
PY
        du -h "$ROOT/dist/tactica-windows.zip"
        ;;
    esac
done

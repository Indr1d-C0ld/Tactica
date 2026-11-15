#!/usr/bin/env bash
# Installa o aggiorna il server di Tactica su Debian (systemd), con Caddy
# davanti per TLS automatico se si indica un dominio.
#
# Prima compila il server come utente normale:
#   cargo build --release -p tactica-server
# Poi, dalla radice del repository:
#   sudo deploy/install.sh --domain gioco.esempio.it
#   sudo deploy/install.sh                 # solo il servizio, senza Caddy
#   deploy/install.sh --dry-run --domain gioco.esempio.it   # mostra cosa farebbe
#
# Lo script si può rilanciare: aggiorna binario e servizio e riavvia.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$ROOT/target/release/tactica-server"
UNIT="$ROOT/deploy/tactica-server.service"
DOMAIN=""
EMAIL=""
DRY=0

usage() {
    sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'
    cat <<'EOF'

Opzioni:
  --domain NOME   dominio pubblico del server (il DNS deve puntare a questa macchina)
  --email INDIRIZZO   contatto per Let's Encrypt (facoltativo)
  --binary PERCORSO   binario da installare (predefinito target/release/tactica-server)
  --dry-run       stampa i comandi senza eseguirli
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
        --domain) DOMAIN="${2:?manca il dominio}"; shift 2 ;;
        --email) EMAIL="${2:?manca l indirizzo email}"; shift 2 ;;
        --binary) BIN="${2:?manca il percorso}"; shift 2 ;;
        --dry-run) DRY=1; shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "opzione sconosciuta: $1" >&2; usage >&2; exit 2 ;;
    esac
done

say() { printf '\033[1m==> %s\033[0m\n' "$*"; }
run() {
    if [ "$DRY" = 1 ]; then printf '    %s\n' "$*"; else "$@"; fi
}
# Scrive su un file di sistema il testo letto da stdin.
write_file() {
    local dest="$1" mode="$2" tmp
    tmp="$(mktemp)"
    cat >"$tmp"
    if [ "$DRY" = 1 ]; then
        printf '    scrive %s:\n' "$dest"
        sed 's/^/      | /' "$tmp"
        rm -f "$tmp"
    else
        install -m "$mode" "$tmp" "$dest"
        rm -f "$tmp"
    fi
}

# --- Controlli ---------------------------------------------------------------
if [ "$DRY" = 0 ] && [ "$(id -u)" != 0 ]; then
    echo "serve root: rilancia con sudo (oppure prova con --dry-run)" >&2
    exit 1
fi
command -v systemctl >/dev/null || { echo "systemd non trovato" >&2; exit 1; }
if [ ! -x "$BIN" ]; then
    echo "binario non trovato: $BIN" >&2
    echo "compilalo prima, come utente normale: cargo build --release -p tactica-server" >&2
    exit 1
fi
if [ -n "$DOMAIN" ] && ! [[ "$DOMAIN" =~ ^[A-Za-z0-9.-]+\.[A-Za-z]{2,}$ ]]; then
    echo "dominio non valido: $DOMAIN" >&2
    exit 1
fi

# --- Servizio ----------------------------------------------------------------
say "Installo il binario e il servizio"
run install -m 755 "$BIN" /usr/local/bin/tactica-server
run install -m 644 "$UNIT" /etc/systemd/system/tactica-server.service
run systemctl daemon-reload
if systemctl is-active --quiet tactica-server; then
    say "Riavvio il servizio con il nuovo binario"
    run systemctl restart tactica-server
else
    run systemctl enable --now tactica-server
fi

if [ "$DRY" = 0 ]; then
    say "Attendo che il server risponda"
    ok=0
    for _ in $(seq 1 20); do
        if { exec 3<>/dev/tcp/127.0.0.1/7777; } 2>/dev/null; then
            printf 'GET /health HTTP/1.0\r\nHost: localhost\r\n\r\n' >&3
            if grep -q '^ok' <&3; then ok=1; fi
            exec 3>&-
            [ "$ok" = 1 ] && break
        fi
        sleep 0.5
    done
    if [ "$ok" = 1 ]; then
        echo "    /health risponde ok"
    else
        echo "il server non risponde: controlla con journalctl -u tactica-server -e" >&2
        exit 1
    fi
fi

# --- Caddy -------------------------------------------------------------------
if [ -z "$DOMAIN" ]; then
    say "Fatto. Il server ascolta solo su 127.0.0.1:7777"
    echo "    per aprirlo su Internet rilancia con --domain NOME"
    exit 0
fi

if ! command -v caddy >/dev/null; then
    say "Installo Caddy dai pacchetti Debian"
    run apt-get update
    run apt-get install -y caddy
fi

say "Configuro Caddy per $DOMAIN"
write_file /etc/caddy/tactica.caddy 644 <<EOF
# Generato da deploy/install.sh: rilancia lo script per cambiarlo.
$DOMAIN {
	encode zstd gzip
	reverse_proxy 127.0.0.1:7777
}
EOF

CADDYFILE=/etc/caddy/Caddyfile
if [ "$DRY" = 1 ] || ! grep -qF "import /etc/caddy/tactica.caddy" "$CADDYFILE" 2>/dev/null; then
    say "Aggiungo l'import del sito a $CADDYFILE (copia di sicurezza in $CADDYFILE.bak)"
    if [ "$DRY" = 1 ]; then
        printf '    aggiunge a %s: import /etc/caddy/tactica.caddy\n' "$CADDYFILE"
    else
        [ -f "$CADDYFILE" ] && cp -a "$CADDYFILE" "$CADDYFILE.bak"
        {
            if [ -n "$EMAIL" ] && ! grep -q '^[[:space:]]*email' "$CADDYFILE" 2>/dev/null; then
                # Il blocco globale deve stare in cima al file.
                printf '{\n\temail %s\n}\n\n' "$EMAIL"
                cat "$CADDYFILE" 2>/dev/null || true
            else
                cat "$CADDYFILE" 2>/dev/null || true
            fi
            printf '\nimport /etc/caddy/tactica.caddy\n'
        } >"$CADDYFILE.new"
        mv "$CADDYFILE.new" "$CADDYFILE"
    fi
fi

run caddy validate --config "$CADDYFILE" --adapter caddyfile
run systemctl enable caddy
run systemctl reload-or-restart caddy

say "Fatto"
cat <<EOF
    Indirizzo per il client: https://$DOMAIN
    Controlli:
      - il DNS di $DOMAIN deve puntare all'IP pubblico di questa macchina;
      - le porte 80 e 443 devono essere aperte (router e firewall);
      - il primo certificato arriva in pochi secondi: journalctl -u caddy -e
EOF

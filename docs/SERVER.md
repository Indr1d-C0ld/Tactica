# Server di gioco

Binario `tactica-server` (crate `server/`). Gestisce account, roster persistente, partite PvE e PvP via WebSocket, replay e statistiche. I dati stanno in SQLite in modalità WAL.

**Il server è l'unica autorità:**

- il client invia solo intenzioni;
- il server le valida con lo stesso motore `tactica-core` e risolve i tiri;
- il seed resta segreto fino a fine partita, così nessun client può prevedere colpi o mancati;
- a fine partita il seed è pubblico nel replay.

## Avvio locale

```bash
cargo run --release -p tactica-server
```

La configurazione si fa con variabili d'ambiente:

| Variabile | Predefinito | Significato |
|---|---|---|
| `TACTICA_ADDR` | `127.0.0.1:7777` | indirizzo di ascolto |
| `TACTICA_DB` | `sqlite://tactica.db` | database (`sqlite::memory:` per le prove) |
| `TACTICA_TURN_SECS` | `60` | tempo per turno; alla scadenza l'IA completa il turno |
| `TACTICA_AI_DELAY_MS` | `400` | pausa tra i turni dell'IA, perché il client possa seguirli |
| `TACTICA_SESSION_DAYS` | `30` | durata dei token di accesso |
| `TACTICA_BACKUP_DIR` | vuoto (backup disattivati) | cartella dei backup (`VACUUM INTO`, coerenti anche a server attivo) |
| `TACTICA_BACKUP_EVERY_HOURS` | `24` | frequenza dei backup |
| `TACTICA_BACKUP_KEEP` | `14` | quanti backup conservare |
| `RUST_LOG` | `info,sqlx=warn` | livello dei log |

## API HTTP

Le rotte indicate con 🔒 richiedono l'header `Authorization: Bearer <token>`.

| Metodo e rotta | Descrizione |
|---|---|
| `POST /api/register` `{username, password}` | crea l'account e restituisce `{token, username}`. Nome: 3-20 caratteri `[A-Za-z0-9_]`; password: almeno 8 caratteri |
| `POST /api/login` | come sopra |
| `POST /api/logout` 🔒 | invalida il token |
| `GET /api/me` 🔒 | profilo, ELO e roster |
| `GET /api/classes` | classi e abilità |
| `GET /api/roster` 🔒 | elenco delle unità del roster |
| `POST /api/roster` 🔒 `{name, class}` | recluta un'unità (massimo 8). Classi: `vanguard`, `scout`, `arcanist`, `mender` |
| `DELETE /api/roster/{id}` 🔒 | congeda un'unità |
| `GET /api/players/{name}` | statistiche personali: vittorie, sconfitte, totali, classi, roster, ultime partite |
| `GET /api/leaderboard?limit=20` | classifica ELO |
| `GET /api/stats` | statistiche globali: classi (uso, % vittorie, medie), biomi, statuti |
| `GET /api/matches/{id}` | replay: setup con seed, azioni, hash finale |
| `GET /health` | `ok` |

Password: hash Argon2id. Token: nel database si salva solo il loro SHA-256.

## Protocollo WebSocket

Connessione a `GET /ws?token=<token>`. Tutti i messaggi sono JSON con il campo `type`.

**Dal client al server:**

```json
{"type":"queue","mode":"pve","units":[1,2,3,4]}
{"type":"queue","mode":"pvp","units":[1,2,3,4]}
{"type":"cancel"}
{"type":"action","action":{"Move":{"x":3,"y":5}}}
{"type":"action","action":{"Act":{"ability":"Flame","target":{"x":6,"y":4}}}}
{"type":"action","action":{"EndTurn":"E"}}
{"type":"leave"}
```

**Dal server al client:**

- `welcome`
- `queued`
- `matched {mode, team, opponent, snapshot}`
- `update {events, snapshot}`: arriva dopo ogni azione, anche dell'IA o dell'avversario
- `error {message}`
- `ended {match_id, winner, result, elo_before, elo_after, rewards}`

Lo `snapshot` è lo stato pubblico completo: mappa, unità, statuti, turno, unità attiva, `moved` e `acted`. Il client può ricostruire una `Battle::from_snapshot` per calcolare caselle raggiungibili, anteprime del danno e suggerimenti dell'IA. Non può risolvere le azioni, perché non conosce il seed.

**Regole di gestione:**

- **Turni:** si accettano solo le azioni della squadra di turno; l'unica eccezione è la resa (`leave`), accettata in ogni momento.
- **Disconnessione o tempo scaduto:** l'IA del server gioca al posto del giocatore, quindi la partita arriva sempre alla fine.
- **Fine partita:**
  - **PvE:** l'avversario IA ha le stesse classi e gli stessi livelli della squadra del giocatore;
  - **PvP:** l'ELO si aggiorna con K=32, a somma zero;
  - **esperienza:** a ogni unità del roster va quella guadagnata in battaglia, più 30 se vince o 10 negli altri casi.

## Deploy su Debian

```bash
cargo build --release -p tactica-server
```

```bash
sudo install -m 755 target/release/tactica-server /usr/local/bin/
```

```bash
sudo install -m 644 deploy/tactica-server.service /etc/systemd/system/
```

```bash
sudo systemctl daemon-reload && sudo systemctl enable --now tactica-server
```

```bash
journalctl -u tactica-server -f
```

Note sul deploy:

- **Isolamento:** il servizio gira con `DynamicUser` e tiene i dati in `/var/lib/tactica` (database e cartella `backup/`).
- **Esposizione su Internet:** metti davanti Caddy usando `deploy/Caddyfile`, così ottieni TLS automatico e il proxy dei WebSocket. Il server ascolta solo su `127.0.0.1`.
- **Aggiornamenti:** ricompila, reinstalla il binario e lancia `sudo systemctl restart tactica-server`. Le migrazioni dello schema partono da sole all'avvio.

## Limiti attuali, prossimi passi

- **Matchmaking:** una sola coda PvP a un posto. Mancano lo scarto ELO e la gestione di più livelli.
- **Livelli in PvP:** il PvP usa i livelli reali del roster; si valuta una modalità classificata con livelli normalizzati.
- **Rate limiting:** manca su login e registrazione; va aggiunto prima di aprire il server su Internet.
- **Progressione:** mancano abilità apprese dall'equipaggiamento, classi sbloccabili, clan e stagioni.

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
| `TACTICA_SEASON_DAYS` | `28` | durata di una stagione classificata |
| `TACTICA_COOP_MIN_PLAYERS` | `2` | giocatori minimi per avviare una missione cooperativa |
| `TACTICA_WAR_MIN_PLAYERS` | `1` | giocatori minimi per lato in una guerra fra clan |
| `RUST_LOG` | `info,sqlx=warn` | livello dei log |

## API HTTP

Le rotte indicate con 🔒 richiedono l'header `Authorization: Bearer <token>`.

| Metodo e rotta | Descrizione |
|---|---|
| `POST /api/register` `{username, password}` | crea l'account e restituisce `{token, username}`. Nome: 3-20 caratteri `[A-Za-z0-9_]`; password: almeno 8 caratteri |
| `POST /api/login` | come sopra |
| `POST /api/logout` 🔒 | invalida il token |
| `GET /api/me` 🔒 | profilo, ELO, monete e roster |
| `GET /api/classes` | classi con requisiti di sblocco, statistiche al livello 1, abilità innate e imparabili (con soglia RP e oggetti che le insegnano) |
| `GET /api/items` | catalogo degli oggetti: slot, classi, bonus, abilità insegnata, prezzo, disponibilità al negozio |
| `GET /api/roster` 🔒 | elenco delle unità del roster |
| `POST /api/roster` 🔒 `{name, class}` | recluta un'unità di classe base (massimo 8), già equipaggiata con il kit iniziale. Classi base: `vanguard`, `scout`, `arcanist`, `mender` |
| `DELETE /api/roster/{id}` 🔒 | congeda un'unità; il suo equipaggiamento torna nell'inventario |
| `PUT /api/roster/{id}/equipment` 🔒 `{items:[...]}` | imposta l'intero equipaggiamento (al massimo un oggetto per slot, adatto alla classe). Tutto o niente: i vecchi oggetti tornano nell'inventario, i nuovi ne vengono prelevati |
| `PUT /api/roster/{id}/class` 🔒 `{class}` | cambia classe se è sbloccata (403 altrimenti, con il requisito); gli oggetti non adatti tornano nell'inventario |
| `GET /api/inventory` 🔒 | monete e oggetti non equipaggiati |
| `POST /api/shop/buy` 🔒 `{item}` | compra un oggetto (solo quelli marcati `in_shop`) |
| `POST /api/shop/sell` 🔒 `{item}` | vende a metà prezzo un oggetto non equipaggiato |
| `GET /api/players/{name}` | statistiche personali: vittorie, sconfitte, totali, classi, roster, ultime partite |
| `GET /api/leaderboard?limit=20` | classifica ELO |
| `GET /api/stats` | statistiche globali: classi (uso, % vittorie, medie), biomi, statuti |
| `GET /api/matches/{id}` | replay: setup con seed, azioni, hash finale e versione delle regole (`rules_version`): il replay si riproduce solo con un motore della stessa versione |
| `GET /api/clans` | elenco dei clan con membri e punti della stagione |
| `POST /api/clans` 🔒 `{name, tag, motto?, open?}` | fonda un clan (costa 200 monete; nome di 3-24 caratteri, sigla di 2-5) |
| `GET /api/clans/{tag}` | dettaglio: membri e ruoli, punti stagionali, storico delle stagioni |
| `PUT /api/clans/{tag}` 🔒 `{motto?, open?}` | modifica il clan (solo il capo) |
| `POST /api/clans/{tag}/join` 🔒 | entra nel clan: libero se aperto, altrimenti serve un invito (massimo 20 membri) |
| `POST /api/clans/{tag}/invite` 🔒 `{username}` | invita un giocatore (capo o ufficiali) |
| `POST /api/clans/{tag}/kick` 🔒 `{username}` | espelle: il capo può espellere chiunque, gli ufficiali solo i membri |
| `POST /api/clans/{tag}/role` 🔒 `{username, role}` | nomina ufficiali o membri; assegnando `leader` il capo cede il comando |
| `POST /api/clans/leave` 🔒 | lascia il clan: il comando passa all'ufficiale più anziano, altrimenti al membro più anziano; l'ultimo che esce scioglie il clan |
| `GET /api/wars` 🔒 | guerre del proprio clan (aperte e ultime concluse) |
| `POST /api/wars` 🔒 `{tag}` | dichiara guerra a un clan (capo o ufficiale) |
| `POST /api/wars/{id}/accept`, `POST /api/wars/{id}/decline` 🔒 | il clan sfidato accetta o rifiuta (capo o ufficiale) |
| `POST /api/wars/{id}/cancel` 🔒 | annulla una guerra non ancora iniziata (capo o ufficiale di uno dei due clan) |
| `GET /api/tournaments` | elenco dei tornei (in corso, con iscrizioni aperte, conclusi) |
| `POST /api/tournaments` 🔒 `{name, max_clans}` | organizza un torneo da 2 a 16 clan (capo o ufficiale); il proprio clan è iscritto |
| `GET /api/tournaments/{id}` | iscritti con teste di serie e tabellone con turni, guerre e vincitori |
| `POST /api/tournaments/{id}/join`, `POST /api/tournaments/{id}/leave` 🔒 | iscrive o ritira il proprio clan finché le iscrizioni sono aperte |
| `POST /api/tournaments/{id}/start`, `POST /api/tournaments/{id}/cancel` 🔒 | il clan organizzatore avvia (almeno 2 clan) o annulla |
| `GET /api/seasons/current` | stagione in corso con classifica dei giocatori e dei clan |
| `GET /api/seasons` | tutte le stagioni |
| `GET /api/seasons/{number}` | classifica finale di una stagione |
| `GET /health` | `ok` |

Password: hash Argon2id. Token: nel database si salva solo il loro SHA-256.

Ogni unità del roster riporta anche:

- `equipment`: le chiavi degli oggetti equipaggiati;
- `usable`: le abilità usabili in battaglia con la configurazione attuale;
- `abilities`: l'avanzamento di ogni abilità (RP, soglia, padroneggiata, in addestramento);
- `unlocked_classes`: le classi a cui l'unità può passare.

## Progressione

- **Classi:**
  - base: Avanguardia, Esploratore, Arcanista, Guaritore;
  - avanzate: Spadaruna e Custode.

  Ogni classe ha Colpo, un'abilità innata e una propria "scuola" di 2 o 3 abilità da imparare.
- **Risonanza:**
  - un oggetto equipaggiato che insegna un'abilità della scuola della classe attuale la rende subito usabile;
  - a fine partita l'abilità accumula punti risonanza (RP): 20 di base, più 10 se si vince, più 2 per azione fino a un massimo di 10 azioni;
  - alla soglia (80-150 RP) l'abilità è padroneggiata: resta usabile senza l'oggetto finché l'unità è in quella classe.
- **Classi avanzate:**
  - **Spadaruna:** richiede il livello 5, 1 abilità di Avanguardia e 1 di Arcanista padroneggiate;
  - **Custode:** richiede il livello 5, 1 abilità di Guaritore e 1 di Avanguardia padroneggiate.

  Le reclute partono sempre da una classe base.
- **Economia:**
  - si parte con 400 monete;
  - ogni partita ne dà 120 se si vince, 70 in caso di pareggio, 40 se si perde;
  - chi vince ha il 50% di probabilità di un bottino, in cui gli oggetti costosi sono più rari. Alcuni oggetti si trovano solo così.
  
  Il bottino si ricava in modo deterministico dal seed della partita e dall'account, quindi è verificabile.
- **IA in PvE:** rispecchia classi, livelli, equipaggiamento e padronanze della squadra del giocatore.

## Status alterati

Le durate si contano nei turni dell'unità colpita: diminuiscono alla fine di ogni suo turno, anche se lo salta dormendo.

| Status | Effetto |
|---|---|
| Veleno | perde l'8% degli HP massimi a fine turno |
| Sonno | salta i turni; i colpi fisici contro di lui vanno sempre a segno; si sveglia se subisce danni |
| Silenzio | niente magie, cure né incantesimi di supporto |
| Lentezza | accumula iniziativa a metà velocità |
| Radici | non può muoversi |
| Rapidità | accumula iniziativa 1,5 volte più in fretta; annulla Lentezza e viceversa |
| Egida | danni fisici subiti ridotti di un terzo |
| Rigenerazione | recupera l'8% degli HP massimi a fine turno |

- **A chi si applicano:** gli status negativi colpiscono solo i nemici, quelli positivi solo gli alleati. Gli attacchi applicano il loro status solo se vanno a segno, con la probabilità indicata.
- **Abilità che infliggono status:**
  - Gelo (Lentezza 30%);
  - Folgore (Radici 25%);
  - Taglio gelido (Lentezza 35%);
  - Ammonimento (Silenzio 40%).
- **Nuove abilità:** tipo "supporto" se non fanno danni né curano.
  - Dardo velenoso (Esploratore);
  - Torpore (Arcanista, Sonno ad area);
  - Egida, Germoglio e Purificazione (Guaritore; Purificazione rimuove gli status negativi);
  - Slancio (Avanguardia, Rapidità attorno a sé);
  - Catene (Custode, Radici).
- **Nuovi oggetti che le insegnano:** Arco del serpente, Verga della quiete, Pendaglio dell'egida, Bastone del germoglio, Veste candida, Corno da battaglia e Flagello. Pendaglio e Corno sono accessori che chiunque può indossare, ma insegnano solo alla classe della scuola.
- **Nuovo statuto:** "Niente alterazioni" (vietato infliggere status negativi).
- **IA:** valuta gli status in base al bersaglio (ad esempio il Silenzio vale soprattutto contro chi lancia incantesimi) ed evita di riapplicare quelli già presenti.
- **Bilanciamento:** con l'equipaggiamento da status contro quello da danno, su 2000 battaglie il risultato è 991 vittorie a 969.

## Clan e stagioni

- **Clan:**
  - ogni account può far parte di un solo clan;
  - i membri ricevono il 10% di monete in più;
  - con le loro partite fanno guadagnare punti stagionali al clan: 3 per una vittoria e 1 per un pareggio in PvP, 1 per una vittoria in PvE.
- **Stagioni:**
  - ce n'è sempre una attiva, con un nome che ruota (Stagione 1 dell'Alba, delle Braci...);
  - il PvP aggiorna, oltre all'ELO di carriera, un punteggio stagionale;
  - alla prima partita di una nuova stagione il punteggio parte da un reset morbido: 1000 + (punteggio precedente - 1000) / 2.
- **Fine stagione:** un controllo ogni minuto chiude le stagioni scadute.
  - **Classifica:** finale per giocatori (solo chi ha giocato) e per clan.
  - **Titoli:** Campione, Podio ed Élite della Stagione N; "Clan campione della Stagione N" ai membri del clan primo.
  - **Monete:** 1000, 600 e 400 ai primi tre; 200 dal 4° al 10° posto; 50 agli altri partecipanti; 300 a ogni membro del clan primo.
  - **Nuova stagione:** si apre subito dopo.
- **`/api/me`:** riporta clan, inviti, titoli e il punteggio stagionale; il profilo pubblico mostra la sigla del clan e i titoli.

## Missioni cooperative di clan

- **Stanze:** da 2 a 6 membri dello stesso clan aprono o raggiungono una stanza di missione e il capostanza la avvia. Il minimo di giocatori si configura con `TACTICA_COOP_MIN_PLAYERS`.
- **Squadra:** la squadra alleata ha 6 posti, divisi fra i giocatori (2 giocatori: 3 unità ciascuno; 3: 2 ciascuno; da 4 a 6: 1 ciascuno). Ognuno comanda solo le proprie unità: il server accetta un'azione solo da chi controlla l'unità di turno.
- **Generazione da seed:**
  - **Nemici:** tanti quanti gli alleati (almeno 3), più 1 per ogni livello di difficoltà oltre il primo. Il loro livello è la media degli alleati più 2 per ogni livello di difficoltà oltre il primo.
  - **Classi:** con difficoltà Ardua e Leggendaria compaiono anche le classi avanzate.
  - **Statuti:** 1 a difficoltà Normale, 2 dalla Ardua in su.
- **Tipi di missione:**
  - **Assedio:** si vince sconfiggendo tutti i nemici.
  - **Caccia:** un campione nominato di 3 livelli più alto; abbatterlo basta per vincere.
- **Abbandono:** se un giocatore se ne va o si disconnette, le sue unità passano all'IA. La squadra si arrende solo quando non resta nessun umano.
- **Ricompense:**
  - esperienza e punti risonanza per le proprie unità;
  - in caso di vittoria, monete pari a 150 + 50 per ogni livello di difficoltà oltre il primo (più il bonus del clan) e bottino al 75%;
  - punti clan: 2 per la vittoria (3 per una caccia), assegnati una sola volta al clan.
- **ELO e punteggio stagionale:** non cambiano.

Messaggi WebSocket:

| Direzione | Messaggio |
|---|---|
| client → server | `room_create {difficulty, units}`, `room_join {room, units}`, `room_leave`, `room_start`, `room_list` |
| server → client | `room {room}` (stato della propria stanza, `null` se non si è in una stanza) e `rooms {rooms}` |

In `matched`:

- **`mine`:** gli indici delle unità comandate;
- **`allies`:** gli altri umani della squadra;
- **`mission`:** nome, difficoltà e tipo della missione.

Lo snapshot contiene `objective` (`Rout` oppure `{"Champion": id}`).

## Guerre fra clan

- **Dichiarazione e risposta:** un capo o ufficiale dichiara guerra a un altro clan; ci può essere una sola guerra aperta fra gli stessi due clan. Un capo o ufficiale del clan sfidato accetta o rifiuta, e finché la guerra non parte ciascuno dei due clan può annullarla.
- **Schieramenti:** dopo l'accettazione i membri dei due clan si schierano via WebSocket. Ogni lato ha 6 posti divisi fra i suoi giocatori; il minimo per lato si configura con `TACTICA_WAR_MIN_PLAYERS` (predefinito 1).
- **Avvio:** la battaglia parte quando i comandanti (capo o ufficiale) di entrambi i lati danno il "pronti". Ogni cambiamento negli schieramenti azzera i "pronti".
- **In battaglia:** la squadra 0 è lo sfidante, la 1 lo sfidato. Chi abbandona passa all'IA; un lato si arrende quando non gli resta nessun umano.
- **Esito:**
  - **ELO dei clan:** parte da 1000 e si aggiorna con K=32, a somma zero, insieme al record di vittorie, sconfitte e pareggi;
  - **punti stagionali:** 5 al clan vincitore;
  - **monete:** 200 a chi vince, 60 a chi perde, 100 in caso di pareggio, più il bonus del clan; bottino normale;
  - **ELO personale e punteggio stagionale personale:** non cambiano.
- **Stati della guerra:** `pending` → `accepted` → `live` → `done`, oppure `declined` o `cancelled`. Dopo un riavvio del server le guerre rimaste `live` tornano `accepted`.

Messaggi WebSocket:

| Direzione | Messaggio |
|---|---|
| client → server | `war_join {war, units}`, `war_leave`, `war_ready {ready}` |
| server → client | `war_room {room}`, con entrambi i lati, i membri e lo stato dei "pronti" |

In `matched` il campo `war` riporta id, sigle e nomi dei due clan.

## Tornei fra clan

- **Formato:** eliminazione diretta con teste di serie in base all'ELO dei clan. Il tabellone è una potenza di due; i posti mancanti diventano turni liberi per le prime teste di serie.
- **Incontri:** ogni incontro è una guerra fra clan creata già accettata, che si gioca con gli schieramenti normali. Gli incontri di torneo non si possono annullare né rifiutare. Quando una guerra finisce il vincitore avanza; in caso di pareggio passa il clan con l'ELO più alto prima della guerra. I turni successivi si creano da soli.
- **Premi:**
  - **campioni:** titolo «Campioni del torneo "nome"», 500 monete a ogni membro e 15 punti stagionali al clan;
  - **finalisti:** titolo e 200 monete a ogni membro.
- **Intestazione:** in battaglia il campo `war.tournament` riporta il nome del torneo.

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
- `ended {match_id, winner, result, elo_before, elo_after, season_before, season_after, clan_points, coins, loot, rewards}`: ogni ricompensa contiene esperienza, livelli e i punti risonanza (`rp`) guadagnati, con `mastered_now` quando un'abilità viene padroneggiata

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
- **Contenuti mancanti:** seconda scuola equipaggiabile, scadenze per gli incontri di torneo (oggi un incontro resta aperto finché non si gioca).

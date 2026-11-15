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
| `TACTICA_TURN_SECS` | `60` | tempo per turno quando più umani sono in partita (PvP, cooperativa, guerre); alla scadenza l'IA completa il turno. Nelle partite in solitaria non c'è limite |
| `TACTICA_AI_DELAY_MS` | `400` | pausa tra i turni dell'IA, perché il client possa seguirli |
| `TACTICA_SESSION_DAYS` | `30` | durata dei token di accesso |
| `TACTICA_BACKUP_DIR` | vuoto (backup disattivati) | cartella dei backup (`VACUUM INTO`, coerenti anche a server attivo) |
| `TACTICA_BACKUP_EVERY_HOURS` | `24` | frequenza dei backup |
| `TACTICA_BACKUP_KEEP` | `14` | quanti backup conservare |
| `TACTICA_SEASON_DAYS` | `28` | durata di una stagione classificata |
| `TACTICA_COOP_MIN_PLAYERS` | `2` | giocatori minimi per avviare una missione cooperativa |
| `TACTICA_WAR_MIN_PLAYERS` | `1` | giocatori minimi per lato in una guerra fra clan |
| `TACTICA_AUTH_PER_MINUTE` | `30` | richieste di accesso o registrazione al minuto per indirizzo IP |
| `TACTICA_REGISTER_PER_HOUR` | `5` | registrazioni all'ora per indirizzo IP |
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
| `PUT /api/roster/{id}/bag` 🔒 `{items:[...]}` | prepara la borsa di battaglia (al massimo 2 consumabili, presi dall'inventario; i precedenti vi tornano) |
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
| `GET /api/campaign` 🔒 | campagna: regioni, tappe con stato (vinta o sbloccata), storia; la chiusura di una regione compare quando la si completa |
| `GET /api/seasons/current` | stagione in corso con classifica dei giocatori e dei clan |
| `GET /api/seasons` | tutte le stagioni |
| `GET /api/seasons/{number}` | classifica finale di una stagione |
| `GET /health` | `ok` |

Password: hash Argon2id. Token: nel database si salva solo il loro SHA-256.

**Limiti ai tentativi** (in memoria, si azzerano al riavvio). Oltre il limite la risposta è `429`:

- per indirizzo IP, richieste di accesso o registrazione al minuto e registrazioni all'ora (variabili nella tabella sopra);
- per nome utente, dopo 5 password sbagliate in 5 minuti l'accesso resta bloccato fino allo scadere dei 5 minuti, anche con la password giusta. Un accesso riuscito azzera il conteggio.

L'indirizzo è quello della connessione. Se la connessione arriva da localhost, cioè da Caddy, conta il primo indirizzo di `X-Forwarded-For`. Caddy scarta quello inviato dal client e lo riscrive, quindi non si può falsificare.

**Coda PvP per ELO:** chi entra in coda accetta avversari con uno scarto di ELO fino a 100 punti. Lo scarto cresce di 50 ogni 10 secondi d'attesa, e dopo 90 secondi si accetta chiunque, perché su un server con pochi giocatori nessuno resti in coda per sempre. A parità di condizioni si sceglie l'avversario con l'ELO più vicino. La coda si riesamina ogni 2 secondi.

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
  
  Il bottino non arriva più a fine partita: si trova solo in battaglia, raggiungendo le unità sconfitte (sezione seguente).
- **IA in PvE:** rispecchia classi, livelli, equipaggiamento e padronanze della squadra del giocatore.

## Bottino e oggetti generati

**Come si trova.** Quando un'unità va KO può lasciare un oggetto sulla sua casella. Le unità KO non bloccano il passaggio. L'oggetto lo raccoglie chi si ferma su quella casella, purché sia della squadra avversaria di chi l'ha lasciato. È l'unico modo di trovare bottino. Gli oggetti raccolti vanno nell'inventario del giocatore che controlla l'unità, qualunque sia l'esito della partita; quelli rimasti a terra si perdono.

| Modalità | Chi lascia oggetti | Probabilità |
|---|---|---|
| PvE | i nemici dell'IA | 45% |
| PvP | entrambe le squadre | 35% |
| Missioni cooperative | i nemici | 55%, rarità più alte con la difficoltà; il campione lascia sempre un oggetto |
| Campagna | i nemici | 50% (tutorial 100%), rarità più alte con la regione; il campione lascia sempre un oggetto |
| Guerre fra clan | entrambe le squadre | 40% |

L'estrazione usa il generatore della battaglia: è deterministica e segreta fino a fine partita, come gli altri tiri. L'IA raccoglie il bottino a portata quando non ha bersagli migliori.

**Oggetti generati** (`core/src/gear.rs`). Più di 200.000 combinazioni, senza contare le abilità insegnate:

- **Rarità:** comune, non comune, raro, epico, leggendario. Le rarità alte hanno più affissi e statistiche migliori.
- **Basi (37):** 16 armi (spade, spadoni, lance, asce, mazze, pugnali, sciabole, archi corti e lunghi, balestre, bacchette, verghe, bastoni, lame runiche, flagelli, tomi), 8 corazze, 6 copricapi e 7 accessori. Ogni base ha le sue classi.
- **Materiali:** sei livelli di qualità secondo il livello di chi lascia l'oggetto (per i metalli: bronzo, ferro, acciaio, argento runico, oricalco, ferro stellare). Moltiplicano le statistiche principali da ×1 a ×4,5.
- **Affissi (39):** aggettivi («affilata», «velenoso», «fiammeggiante»...) e complementi («del Lupo», «della Salamandra»...). Danno statistiche oppure effetti:
  - al colpo, sui colpi fisici: veleno, lentezza, radici, silenzio o sonno;
  - assorbimento di una parte dei danni inflitti;
  - attacchi fisici di un elemento (fuoco, ghiaccio, fulmine, luce);
  - resistenza a un elemento (danni dimezzati);
  - immunità a uno status;
  - rigenerazione a fine turno;
  - +1 movimento o salto.
- **Abilità insegnate:** i rari (30%) e gli epici (65%) possono insegnare un'abilità della scuola di una classe che può usarli.
- **Leggendari (16):** oggetti unici con nome, storia ed effetti propri, per esempio «Lama dell'Alba» o «Eco di Varen».

Un oggetto generato è descritto per intero dalla sua chiave, per esempio `g.spa.2.r.aff.lup.-.sweep` oppure `u.alba.3`. Inventario, equipaggiamento e replay lo salvano come una stringa qualsiasi. `GET /api/inventory` restituisce per ogni oggetto la scheda completa: nome, rarità, slot, classi, bonus, effetti descritti, abilità insegnata, icona e storia.

**Equipaggiamento:** quattro slot (arma, armatura, copricapo, accessorio), al massimo un oggetto per slot.

## Consumabili e borsa di battaglia

Ogni unità porta in battaglia una **borsa** con al massimo 2 consumabili, preparata dal roster. Usare un consumabile è l'azione del turno (al posto di un'abilità); quelli usati spariscono dalla borsa a fine partita, gli altri restano.

| Tipo | Consumabili | Effetto |
|---|---|---|
| Pozioni e preparati | Pozione, Etere, Rimedio, Essenza vitale, Elisir | HP, MP, rimozione degli status negativi, rianimazione di un alleato sconfitto accanto (se la sua casella è libera), HP al massimo |
| Da lancio | Bomba di brace, Fiala di gelo, Fiala tossica, Polvere del sonno | danni di fuoco ad area, ghiaccio e Lentezza, Veleno ad area, Sonno ad area (portata fino a 4) |
| Pergamene (23) | Fiamma, Gelo, Folgore, Vampa, Luce, Giudizio, Catene, Torpore, Cura, Cura maggiore, Santuario, Egida, Germoglio, Purificazione, Slancio, Bufera, Temporale, Meteora, Silenzio arcano, Preghiera, Raggio sacro, Ritorno, Voto | lanciano l'incantesimo una volta, da qualsiasi classe, senza MP, con una MAG minima garantita dalla qualità |

- **Qualità:** I-VI, secondo il livello di chi lascia il consumabile. Aumenta i valori fissi (da ×1 a ×4,5) e la MAG minima delle pergamene (18 + 7 per livello di qualità).
- **Regole:** valgono gli statuti (una pozione viola «Niente cure», una pergamena di Fiamma «Niente fuoco»). Il Silenzio blocca le pergamene ma non pozioni e bombe.
- **Bottino:** circa un oggetto su tre è un consumabile, della rarità estratta.
- **Negozio:** Pozione I, Etere I, Rimedio I, Bomba di brace I e Pergamena di Cura I.
- **IA:** usa i consumabili quando rendono più del loro costo stimato; rianima appena può.

## Abilità per scuola

Ogni classe ha Colpo, un'abilità innata e una scuola di abilità da imparare con l'equipaggiamento (punti risonanza fino alla padronanza). Quelle in corsivo sono la seconda serie, aggiunta con il bottino generato.

| Classe | Innata | Scuola |
|---|---|---|
| Avanguardia | Fendente | Ampio fendente, Carica, Slancio, *Colpo alle gambe* (Radici), *Baluardo* (Egida attorno a sé), *Squarcio* |
| Esploratore | Tiro | Tiro mirato, Raffica, Dardo velenoso, *Freccia perforante* (portata 7), *Freccia del silenzio*, *Trappola di rovi* (Radici ad area) |
| Arcanista | Fiamma | Gelo, Folgore, Vampa, Torpore, *Bufera* (ghiaccio, area 2), *Temporale*, *Silenzio arcano*, *Meteora* |
| Guaritore | Cura | Luce, Cura maggiore, Egida, Germoglio, Purificazione, *Preghiera* (cura ad area 2), *Raggio sacro*, *Ritorno* (rianima un alleato con il 25% degli HP) |
| Spadaruna | Lama runica | Lama vorace, Taglio gelido, *Lama di fiamma*, *Lama tonante* (Radici), *Sigillo runico* (Silenzio) |
| Custode | Ammonimento | Santuario, Giudizio, Catene, *Muro di scudi* (Egida, area 2), *Martello di luce*, *Voto* (Rigenerazione) |

Gli oggetti rari ed epici possono insegnare qualsiasi abilità della scuola di una classe che li può usare. Con l'esempio `balance` e `TACTICA_MASTER=2` (tutte le abilità padroneggiate) le squadre speculari restano in equilibrio, 284 vittorie a 276; con `TACTICA_MASTER=1` la squadra che padroneggia la scuola vince circa il 63% delle volte.

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
  - in caso di vittoria, monete pari a 150 + 50 per ogni livello di difficoltà oltre il primo (più il bonus del clan);
  - il bottino raccolto in battaglia da ciascuno;
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
  - **monete:** 200 a chi vince, 60 a chi perde, 100 in caso di pareggio, più il bonus del clan; bottino raccolto in battaglia;
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

## Campagna

- **Tappe:** 15 tappe fisse in 5 regioni (`server/src/campaign.rs`), sbloccate una alla volta.
- **Avvio:** dal client si avvia una tappa con `{"type":"campaign","node":"r1n2","units":[...]}`. La battaglia usa il bioma della regione, con nemici dal generatore di missioni e livello pari alla media della squadra più il bonus della tappa. L'ultima tappa di ogni regione è la caccia a un campione; la prima è il tutorial, con 2 nemici e 1 statuto.
- **Vittoria:** registra la tappa, dà le monete delle missioni (più il bonus del clan), e 1 punto al clan. Il bottino è quello raccolto in battaglia.

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
- `ended {match_id, winner, result, elo_before, elo_after, season_before, season_after, clan_points, coins, loot, rewards}`: `loot` è l'elenco delle schede degli oggetti raccolti dalle proprie unità; ogni ricompensa contiene esperienza, livelli e i punti risonanza (`rp`) guadagnati, con `mastered_now` quando un'abilità viene padroneggiata

Lo `snapshot` è lo stato pubblico completo: mappa, unità, statuti, turno, unità attiva, `moved` e `acted`. Il client può ricostruire una `Battle::from_snapshot` per calcolare caselle raggiungibili, anteprime del danno e suggerimenti dell'IA. Non può risolvere le azioni, perché non conosce il seed.

**Regole di gestione:**

- **Turni:** si accettano solo le azioni della squadra di turno; l'unica eccezione è la resa (`leave`), accettata in ogni momento.
- **Disconnessione o tempo scaduto:** l'IA del server gioca al posto del giocatore, quindi la partita arriva sempre alla fine.
- **Fine partita:**
  - **PvE:** l'avversario IA ha le stesse classi e gli stessi livelli della squadra del giocatore;
  - **PvP:** l'ELO si aggiorna con K=32, a somma zero;
  - **esperienza:** a ogni unità del roster va quella guadagnata in battaglia, più 30 se vince o 10 negli altri casi.

## Deploy su Debian

Sul server, come utente normale, compila il binario:

```bash
cargo build --release -p tactica-server
```

Poi installa servizio e Caddy (TLS automatico con Let's Encrypt) con lo script, dalla radice del repository:

```bash
sudo deploy/install.sh --domain gioco.esempio.it --email tua@email.it
```

Lo script:

1. installa il binario in `/usr/local/bin` e il servizio `tactica-server`, poi lo avvia o lo riavvia;
2. aspetta che `/health` risponda;
3. con `--domain`, installa Caddy dai pacchetti Debian se manca, scrive il sito in `/etc/caddy/tactica.caddy` e lo importa da `/etc/caddy/Caddyfile` (con una copia `Caddyfile.bak`), poi valida la configurazione e ricarica Caddy.

Senza `--domain` installa solo il servizio, raggiungibile da `127.0.0.1:7777`. Con `--dry-run` mostra i comandi senza eseguirli e non serve `sudo`.

Prima di aprire il server su Internet:

- il DNS del dominio deve puntare all'IP pubblico della macchina;
- le porte 80 e 443 devono essere aperte sul router e sul firewall. La porta 7777 resta chiusa: il server ascolta solo su `127.0.0.1`.

Note sul deploy:

- **Isolamento:** il servizio gira con `DynamicUser` e tiene i dati in `/var/lib/tactica` (database e cartella `backup/`).
- **Aggiornamenti:** `git pull`, ricompila e rilancia lo script; le migrazioni dello schema partono da sole all'avvio.
- **Log:** `journalctl -u tactica-server -f` (server) e `journalctl -u caddy -e` (certificati).
- **Configurazione:** per cambiare una variabile senza toccare il file del servizio usa `sudo systemctl edit tactica-server` e aggiungi righe `Environment=NOME=valore` nella sezione `[Service]`.

## Limiti attuali, prossimi passi

- **Livelli in PvP:** il PvP usa i livelli reali del roster; si valuta una modalità classificata con livelli normalizzati.
- **Contenuti mancanti:** seconda scuola equipaggiabile, scadenze per gli incontri di torneo (oggi un incontro resta aperto finché non si gioca).

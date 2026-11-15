# Client Godot

Il progetto Godot 4.7 è in `client/`; l'estensione Rust (GDExtension) che porta `tactica-core` dentro Godot è in `client-ext/`.

La grafica è pixel-art originale in stile tattico portatile, generata da `tools/pixelart` (vedi il suo README):

- **Personaggi:** chibi animati nelle 4 direzioni, con posa d'azione, KO, colori di squadra e aspetto variabile.
- **Mappe:** caselle isometriche con fianchi stratificati, acqua e lava animate, vegetazione in rilievo.
- **Interfaccia:** cornici dorate.

La mappa si disegna in coordinate native e si ingrandisce per un fattore intero con filtro "nearest", così i pixel restano nitidi. La rotella cambia lo zoom e il tasto centrale trascinato sposta la mappa.

![Battaglia](img/battaglia.png)
![Roster](img/roster.png)

## Avvio

```bash
cargo build -p tactica-godot
```

```bash
godot --path client
```

L'estensione viene caricata da `target/debug/libtactica_godot.so`, oppure da `target/release/` nelle build di rilascio; i percorsi sono in `client/tactica.gdextension`. Va ricompilata dopo ogni modifica al core.

Per aprire il progetto nell'editor:

```bash
godot --path client --editor
```

## Schermate

- **Accesso:** indirizzo del server, accesso o registrazione, schermaglia offline contro l'IA senza server.
- **Menu:**
  - **Gioca:** scelta di fino a 4 unità, coda PvE o PvP, annullamento della coda.
  - **Roster:** reclutamento; per ogni unità equipaggiamento per slot, abilità usabili, progressi di risonanza, cambio classe con i requisiti mancanti, congedo.
  - **Negozio:** acquisti e vendite.
  - **Statistiche:** profilo, classifica, statistiche globali.
- **Battaglia:**
  - **Mappa:** isometrica, scalata alla finestra.
  - **Pannello sinistro:** scheda dell'unità attiva (o di quella sotto il mouse) e azioni: muovi, abilità con costo in MP, fine turno con scelta della direzione, suggerimento dell'IA, abbandono.
  - **Anteprima al passaggio sul bersaglio:** area d'effetto, danni e probabilità di colpire per ogni unità, avviso se l'azione viola uno statuto.
  - **Registro degli eventi** in italiano, con numeri fluttuanti per danni, cure, mancati, sanzioni e livelli.
  - **Fine partita:** risultato, ELO, monete, bottino, esperienza e punti risonanza per unità.

Comandi: clic sinistro per scegliere la casella, clic destro per annullare la modalità corrente, rotella per lo zoom, tasto centrale per spostare la mappa.

## Divisione dei compiti

- **Partite online:** il client invia solo intenzioni e riceve eventi più lo snapshot pubblico. Con `TacticaEngine.from_snapshot` calcola in locale caselle raggiungibili, bersagli, anteprime e suggerimenti; la risoluzione resta al server.
- **Schermaglia offline:** `TacticaEngine.new_local` crea una battaglia completa.
- **Scambio dati:** i dati strutturati passano come testo JSON. Snapshot ed eventi vengono letti dal testo grezzo del messaggio (`set_snapshot_from_message`, `describe_message_events`), così i numeri non passano dalle conversioni float di GDScript.

## Modalità automatica (test e dimostrazioni)

Argomenti dopo `--`:

| Argomento | Effetto |
|---|---|
| `--autoplay` | gioca da solo con i suggerimenti dell'IA |
| `--offline` | schermaglia locale |
| `--server=URL` | server da usare |
| `--user=… --pass=…` | credenziali; se l'account non esiste viene registrato |
| `--mode=pve` oppure `--mode=pvp` | modalità della partita |
| `--shots=CARTELLA` | salva schermate dei momenti salienti |

In modalità online, se il roster è vuoto il client:

1. recluta una squadra;
2. compra e equipaggia la Verga di brina;
3. fotografa le schede del menu;
4. gioca;
5. mostra le ricompense e chiude.

Esempi:

```bash
godot --path client -- --autoplay --offline --shots=/tmp/shots
```

```bash
godot --path client -- --autoplay --server=http://127.0.0.1:7777 --user=prova --pass=password-lunga
```

Per una prova senza finestra (niente schermate):

```bash
godot --headless --path client -- --autoplay --offline
```

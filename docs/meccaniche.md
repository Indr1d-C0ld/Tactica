# Meccaniche di combattimento

Il motore di Tactica (`core/`) è progettato da zero: formule, valori e nomi sono nostri. Le meccaniche riprendono schemi comuni nei tattici a turni su griglia isometrica, reinterpretati e calibrati con simulazioni.

## Calibrazione

```bash
cargo run --release -p tactica-core --example balance 1000
```

Con l'esempio `balance` le squadre speculari vincono circa 50 e 50 (su 1000 battaglie 485 a 460, 55 pareggi) e le partite durano in media 84 turni. Con `TACTICA_STATUS=1` una squadra equipaggiata per gli status alterati affronta una squadra da danno puro: su 2000 battaglie finisce 991 a 969.

## Riepilogo

| Meccanica | Tactica | Note |
|---|---|---|
| Ordine dei turni | iniziativa (CT) che cresce con la velocità; il turno arriva a 100; muoversi e agire costano | schema comune nei tattici a turni |
| Danno fisico | `ATK × potenza / 100 × 2`, ridotto da `100 / (100 + 5 × DEF)`, ±10% per livello di dislivello (massimo 3), variazione casuale ±10% | |
| Occupazione delle caselle | ogni unità in piedi occupa la sua casella: nessuno può fermarsi su una casella occupata né attraversarla, che sia alleato o nemico. Le unità KO non bloccano | |
| Terreno | roccia DEF +3; boscaglia schivata +15%; acqua schivata −10% e 2 punti movimento; sabbia 2 punti movimento; lava −10% HP a fine turno | il client mostra gli effetti della casella sotto il cursore |
| Colpire | 90% + fianco (15%) o spalle (30%) − schivata (unità e terreno) | |
| Magia | `MAG × potenza / 100 × 2` ridotto dalla RES; elementi influenzati dal terreno (fulmine sull'acqua, ghiaccio sulla lava) | |
| Abilità imparate dall'equipaggiamento | punti risonanza a fine battaglia; padronanza permanente | dettagli in [SERVER.md](SERVER.md#progressione) |
| Statuti di battaglia | 9 statuti estratti a caso; chi li viola viene sospeso e poi espulso | |
| Status alterati | 8 status, durate nei turni dell'unità | dettagli in [SERVER.md](SERVER.md#status-alterati) |
| Bottino | le unità KO possono lasciare un oggetto; lo raccoglie l'avversario che si ferma sulla casella | dettagli in [SERVER.md](SERVER.md#bottino-e-oggetti-generati) |
| Effetti degli oggetti | elemento degli attacchi fisici, status al colpo (solo se il colpo va a segno), assorbimento, resistenze (danni dimezzati), immunità, rigenerazione | si sommano fra tutti gli oggetti equipaggiati |

## Mappe

- **Dimensioni:** da 14×12 a 20×17 caselle, in media circa 220; altezze fino a 8 livelli (11 sull'altopiano).
- **Rilievo:** rumore su tre scale (forme ampie, medie e piccole) più una variazione casuale secondo la rugosità.
- **Forme del terreno:** ogni mappa ne estrae una, con pesi diversi per bioma:

  | Forma | Descrizione |
  |---|---|
  | Colline | rilievo morbido |
  | Vallata | due crinali e un corridoio fra le squadre |
  | Colle | rilievo o mesa al centro |
  | Gola | corridoio basso fra pareti ripide |
  | Gradoni | terrazze larghe, salti di due livelli |
  | Fiume | corso d'acqua (o colata di lava) fra le squadre, con due guadi |
  | Laghi | stagni sparsi |
  | Pinnacoli | colonne di roccia isolate |

- **Dettagli:** affioramenti di roccia sparsi e, nei biomi verdi, macchie di boscaglia.
- **Verifica:** ogni mappa deve permettere a tutte le unità di raggiungersi a piedi; altrimenti se ne genera un'altra.
- **Anteprima nel terminale:** `cargo run -p tactica-core --example maps 4 SEED`.

Con queste mappe le partite simulate durano in media 84 turni.

## Determinismo

- Il motore usa solo interi e un generatore pseudocasuale SplitMix64 con seed.
- A parità di seed e di azioni lo stato finale è identico, verificato con un hash.
- Lo stesso motore gira sul server (che è l'autorità), nel client (anteprime, suggerimenti, schermaglia offline) e nei test. Per questo i replay si riproducono esattamente.
- `RULES_VERSION` (oggi 3) cambia quando cambiano le regole: un replay si riproduce solo con un motore della stessa versione.

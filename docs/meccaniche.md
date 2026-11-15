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
| Movimento | MOV e salto dipendono da classe, popolo, status ed equipaggiamento; il terreno ha un costo per unità | sezione «Movimento e popoli» |
| Occupazione delle caselle | ogni unità in piedi occupa la sua casella: nessuno può fermarsi su una casella occupata né attraversarla, che sia alleato o nemico. Le unità KO non bloccano | |
| Terreno | roccia DEF +3; boscaglia schivata +15%; acqua schivata −10% e 2 punti movimento; sabbia 2 punti movimento; lava −10% HP a fine turno | il client mostra gli effetti della casella sotto il cursore |
| Colpire | 90% + fianco (15%) o spalle (30%) − schivata (unità e terreno) | |
| Magia | `MAG × potenza / 100 × 2` ridotto dalla RES; elementi influenzati dal terreno (fulmine sull'acqua, ghiaccio sulla lava) | |
| Abilità imparate dall'equipaggiamento | punti risonanza a fine battaglia; padronanza permanente | dettagli in [SERVER.md](SERVER.md#progressione) |
| Statuti di battaglia | 9 statuti estratti a caso; chi li viola viene sospeso e poi espulso | |
| Status alterati | 8 status, durate nei turni dell'unità | dettagli in [SERVER.md](SERVER.md#status-alterati) |
| Bottino | le unità KO possono lasciare un oggetto; lo raccoglie l'avversario che si ferma sulla casella | dettagli in [SERVER.md](SERVER.md#bottino-e-oggetti-generati) |
| Consumabili | borsa da 2 posti per unità; usarne uno è l'azione del turno; pergamene con MAG minima garantita | dettagli in [SERVER.md](SERVER.md#consumabili-e-borsa-di-battaglia) |
| Effetti degli oggetti | elemento degli attacchi fisici, status al colpo (solo se il colpo va a segno), assorbimento, resistenze (danni dimezzati), immunità, rigenerazione | si sommano fra tutti gli oggetti equipaggiati |

## Movimento e popoli

**Movimento (MOV)** e **salto** di base dipendono dalla classe:
- **MOV:** 5 per Esploratore e Lama d'ombra, 3 per Arcanista, Custode e Invocatore, 4 per le altre classi;
- **salto:** 5 il Dragone, 3 Esploratore, Lama d'ombra e Geomante, 2 le altre classi.

Su questa base agiscono:

| Fonte | Effetto |
|---|---|
| Rapidità / Lentezza | MOV +1 / −1 (oltre all'iniziativa) |
| Zavorra (nuovo status) | MOV −2, salto −1; da Sabbie mobili del Geomante e Fiala di piombo |
| Volo (nuovo status) | ignora dislivelli e costi del terreno; da Ali del drago (Dragone) e Piuma del vento |
| Radici | nessun movimento |
| Corazza a piastre | salto −1 |
| Oggetti | «del Viandante» MOV +1, «dello Stambecco» salto +1 |
| Classe | Esploratore e Lama d'ombra non affondano nella sabbia; il Geomante paga 1 su ogni terreno; il Dragone scende da qualsiasi altezza |

Il salto non scende mai sotto 2 (lo scalino che le mappe garantiscono sempre percorribile); solo la Zavorra, temporanea, può portarlo a 1. Il MOV è almeno 1.

**Costo del terreno:** 1 punto, 2 per acqua, sabbia e boscaglia.

**Popoli** (si scelgono alla recluta):

| Popolo | Tratti |
|---|---|
| Umano | ATK, DEF, MAG e RES +1 |
| Silvano | MOV +1; la boscaglia costa 1 e dà schivata +10; DEF −2, HP −5% |
| Roccioso | MOV −1, salto −1; HP +15%, DEF +3; la lava non li brucia e costa 1 |
| Alato | salto +2; l'acqua costa 1; schivata +5; HP −5% |
| Ondino | l'acqua costa 1 e dà schivata +15; la sabbia costa 3; MAG +2 |

Nelle simulazioni con classi e popoli casuali (`--example classes`) ogni popolo vince fra il 43% e il 48% delle battaglie (al netto dei pareggi), ogni classe fra il 38% e il 53%. Per ora i popoli non hanno sprite propri: si riconoscono dal nome nella scheda dell'unità.

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

Con queste mappe le partite simulate durano in media 85-95 turni.

## Determinismo

- Il motore usa solo interi e un generatore pseudocasuale SplitMix64 con seed.
- A parità di seed e di azioni lo stato finale è identico, verificato con un hash.
- Lo stesso motore gira sul server (che è l'autorità), nel client (anteprime, suggerimenti, schermaglia offline) e nei test. Per questo i replay si riproducono esattamente.
- `RULES_VERSION` (oggi 3) cambia quando cambiano le regole: un replay si riproduce solo con un motore della stessa versione.

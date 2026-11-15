# Meccaniche di combattimento

Il motore di Tactica (`core/`) è progettato da zero: formule, valori e nomi sono nostri. Le meccaniche riprendono schemi comuni nei tattici a turni su griglia isometrica, reinterpretati e calibrati con simulazioni.

## Calibrazione

```bash
cargo run --release -p tactica-core --example balance 1000
```

Con l'esempio `balance` le squadre speculari vincono circa 50 e 50 e le partite durano in media 60-75 turni. Con `TACTICA_STATUS=1` una squadra equipaggiata per gli status alterati affronta una squadra da danno puro: su 2000 battaglie finisce 991 a 969.

## Riepilogo

| Meccanica | Tactica | Note |
|---|---|---|
| Ordine dei turni | iniziativa (CT) che cresce con la velocità; il turno arriva a 100; muoversi e agire costano | schema comune nei tattici a turni |
| Danno fisico | `ATK × potenza / 100 × 2`, ridotto da `100 / (100 + 5 × DEF)`, ±10% per livello di dislivello (massimo 3), variazione casuale ±10% | |
| Colpire | 90% + fianco (15%) o spalle (30%) − schivata (unità e terreno) | |
| Magia | `MAG × potenza / 100 × 2` ridotto dalla RES; elementi influenzati dal terreno (fulmine sull'acqua, ghiaccio sulla lava) | |
| Abilità imparate dall'equipaggiamento | punti risonanza a fine battaglia; padronanza permanente | dettagli in [SERVER.md](SERVER.md#progressione) |
| Statuti di battaglia | 9 statuti estratti a caso; chi li viola viene sospeso e poi espulso | |
| Status alterati | 8 status, durate nei turni dell'unità | dettagli in [SERVER.md](SERVER.md#status-alterati) |

## Determinismo

- Il motore usa solo interi e un generatore pseudocasuale SplitMix64 con seed.
- A parità di seed e di azioni lo stato finale è identico, verificato con un hash.
- Lo stesso motore gira sul server (che è l'autorità), nel client (anteprime, suggerimenti, schermaglia offline) e nei test. Per questo i replay si riproducono esattamente.
- `RULES_VERSION` cambia quando cambiano le regole: un replay si riproduce solo con un motore della stessa versione.

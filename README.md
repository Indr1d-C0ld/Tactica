# Gioco tattico multiplayer (nome di lavoro: Tactica)

Gioco tattico fantasy originale, ispirato ai classici tattici a turni per console portatili. Contiene:

- battaglie su griglia con altezze;
- classi;
- statuti procedurali con sanzioni;
- mappe generate da seed;
- un server autoritativo con statistiche persistenti.


## Stato

- `core/`: motore di regole deterministico (solo aritmetica intera, nessuna dipendenza). Comprende:
  - mappe procedurali per 5 biomi, con verifica che gli schieramenti siano collegati;
  - 6 classi (4 base, 2 avanzate da sbloccare);
  - 20 abilità;
  - 22 oggetti che danno bonus e insegnano abilità, padroneggiabili con i punti risonanza;
  - turni a CT;
  - danno con altezza, lato d'attacco e terreno;
  - 8 statuti con sospensione ed espulsione;
  - esperienza e livelli;
  - contatori statistici per unità;
  - IA golosa che rispetta gli statuti;
  - replay (seed più azioni).
- `cli/`: demo da terminale con una battaglia tra IA e la verifica del replay.
- `server/`: server autoritativo (axum, WebSocket, SQLite WAL). Comprende:
  - account Argon2id;
  - roster persistente con esperienza, livelli, equipaggiamento e padronanze;
  - inventario, negozio e bottino;
  - partite PvE contro l'IA e PvP con ELO;
  - IA che subentra a chi si disconnette o va oltre il tempo;
  - replay verificabili;
  - statistiche personali e globali;
  - backup automatici.

  Dettagli in [docs/SERVER.md](docs/SERVER.md).
- `deploy/`: unit systemd isolata e Caddyfile per TLS.

## Comandi

```bash
cargo test
```

```bash
cargo run --release -p tactica-cli -- 12345
```

```bash
cargo run --release -p tactica-core --example balance 1000
```

```bash
cargo run --release -p tactica-server
```

## Prossimi passi

1. **Clan e stagioni**, status alterati, matchmaking per ELO; rate limiting prima dell'apertura su Internet.
2. **Client Godot 4** con godot-rust, che riusa `core/`.

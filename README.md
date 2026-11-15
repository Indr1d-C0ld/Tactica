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
  - 4 classi;
  - 9 abilità;
  - turni a CT;
  - danno con altezza, lato d'attacco e terreno;
  - 8 statuti con sospensione ed espulsione;
  - esperienza e livelli;
  - contatori statistici per unità;
  - IA golosa che rispetta gli statuti;
  - replay (seed più azioni).
- `cli/`: demo da terminale con una battaglia tra IA e la verifica del replay.

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

## Prossimi passi

1. **Server.** Rust con tokio e axum, WebSocket e PostgreSQL; account, lobby e partite autoritative.
2. **Progressione persistente.** Abilità apprese dall'equipaggiamento, classi sbloccabili, profilo e statistiche globali.
3. **Client Godot 4** con godot-rust, che riusa `core/`.

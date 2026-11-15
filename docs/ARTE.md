# Grafica: specifiche per il pixel artist

Tutta la grafica del gioco è originale e generata da `tools/pixelart` (vedi il suo README). Questa pagina spiega come sostituirla con grafica disegnata a mano senza toccare il codice del client.

## Come si sostituisce un atlante

Il client cerca ogni immagine in quest'ordine:

1. **Prova immediata:** `user://override/NOME.png`, letta all'avvio senza reimportare. Su Linux la cartella è `~/.local/share/godot/app_userdata/Tactica/override/`.
2. **Sostituzione definitiva:** `client/assets/override/NOME.png`. Va inclusa nel progetto e poi importata con `godot --headless --path client --import`.
3. **Altrimenti:** la grafica generata in `client/assets/NOME.png`.

Le dimensioni e la disposizione delle celle devono restare quelle indicate qui sotto. Tutte le immagini sono PNG RGBA; il trasparente è alfa 0. Il gioco le ingrandisce solo per multipli interi, con filtro "nearest".

## Personaggi: un solo foglio base

L'atlante `units.png` contiene 180 varianti (6 classi × 2 squadre × 5 colori di capelli × 3 carnagioni). L'artista non deve disegnarle tutte: basta un foglio base, e lo script ricolora tutto il resto.

- **Foglio base:** 352×192 pixel, cioè 11 fotogrammi × 6 classi in celle 32×32.
- **Righe, nell'ordine:** Avanguardia, Esploratore, Arcanista, Guaritore, Spadaruna, Custode.
- **Colonne:** 0-1 fronte a riposo; 2-3 retro a riposo; 4 fronte in azione; 5 retro in azione; 6 KO; 7-8 fronte in cammino; 9-10 retro in cammino.
- **Viste:** il "fronte" guarda in basso a destra, il "retro" in alto a destra. Il gioco specchia gli sprite per le altre due direzioni.
- **Appoggio dei piedi:** pixel (16, 30) di ogni cella. Il personaggio è alto circa 28 pixel.
- **Colori chiave:** sono riservati e vengono sostituiti dalle palette del gioco. Ogni gruppo va dal tono più scuro al più chiaro.

  | Uso | Colori chiave |
  |---|---|
  | Squadra (stoffe, cimieri, scudi) | `#400040` `#800080` `#c000c0` `#ff40ff` |
  | Capelli | `#004040` `#008080` `#00c0c0` `#40ffff` |
  | Pelle | `#404000` `#808000` `#c0c000` `#ffff40` |

  Si possono usare anche le due varianti di contorno che il generatore ricava dal tono più scuro di ciascun gruppo, scurito al 55% e al 78%. Tutti gli altri colori restano invariati.

Punti di partenza:
- **`tools/pixelart/templates/units_base.png`:** il foglio base attuale, da ridipingere.
- **`units_base_guide.png`:** la stessa immagine ingrandita 4×, con griglia, numeri dei fotogrammi e punto d'appoggio.

Per generare l'atlante completo dal foglio disegnato:

```bash
python3 tools/pixelart/artkit.py build percorso/units_base.png
```

Lo script scrive `client/assets/override/units.png`. Ho verificato che il foglio base generato, ricolorato così, riproduce l'atlante del gioco identico pixel per pixel.

## Altri atlanti

| File | Formato |
|---|---|
| `terrain3d.png` | celle 16×16. Righe per terreno: Erba, Pietra, Sabbia, Acqua, Lava, Boscaglia. Colonne 0-2: sommità (varianti; per acqua e lava, tre fotogrammi d'animazione). Colonna 3: fianco (8 righe per livello). Colonna 4: fianco del livello più alto, con il bordo che ricade. Riga 6: cespuglio A, cespuglio B, sassi, ciuffo d'erba |
| `terrain.png` | celle 32×32; dalla riga 6 servono la freccia sopra l'unità attiva (colonna 2) |
| `vfx.png` | celle 32×32, 6 fotogrammi per riga. Righe: slash, arrow (proiettile nel fotogramma 0), impact, fire, ice, thunder, holy, heal, buff, debuff, poison, drain. Le durate sono in `tools/pixelart/vfx.py` (`LENGTHS`) |
| `backdrop.png` | 480×270, fondale dietro la mappa (riempie lo schermo mantenendo le proporzioni) |
| `worldmap.png` | 480×270, pergamena della mappa del mondo; le tappe sono in coordinate 0..1 (`server/src/campaign.rs`) |
| `ui_window.png` | 32×32, cornice a 9 parti con margine 8 |
| `ui_button*.png`, `ui_field.png` | 24×24, cornici a 9 parti con margine 6 |
| `fonts/tactica.fnt` e `.png` | font BMFont con cella di 11 pixel (righe 0-1 per gli accenti delle maiuscole, linea di base sotto la riga 8) |

Per audio e musica vale lo stesso principio: i file in `client/assets/audio/` si possono sostituire con WAV dallo stesso nome. Il loop delle musiche copre l'intero file.

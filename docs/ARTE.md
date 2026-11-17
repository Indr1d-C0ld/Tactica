# Grafica: specifiche per il pixel artist

Tutta la grafica del gioco è originale. Il font e la grafica di riserva sono generati da `tools/pixelart` (vedi il suo README). Personaggi (anche per popolo), ritratti, icone, terreni, interfaccia, effetti, fondale e mappa del mondo sono fatti con [PixelLab](https://pixellab.ai): le immagini scaricate sono nel repository e il gioco non ha bisogno di PixelLab per funzionare. Questa pagina spiega anche come sostituire la grafica con altra disegnata a mano, senza toccare il codice del client.

## Grafica fatta con PixelLab

Secondo i termini di servizio di PixelLab (sezione 3.3), le immagini generate appartengono a chi le crea e si possono usare e distribuire per qualsiasi scopo, tranne l'addestramento di altri modelli. Per questo sono incluse nel repository sotto la licenza del progetto.

**Personaggi** (`tools/pixelart/pixellab/<classe>/pack/`):

- un personaggio per classe, alto 32 pixel, in 8 direzioni;
- animazioni: camminata (fronte e retro), caduta e un'azione propria della classe (fendente, tiro con l'arco, incantesimo...);
- il blu dei vestiti diventa il colore della squadra.

**Popoli** (`tools/pixelart/pixellab/people/<popolo>/<classe>/pack/`): le stesse dodici classi rifatte per Silvani (orecchie a punta, pelle verde oliva), Rocciosi (pelle di pietra screpolata, occhi ambra), Alati (ali piumate) e Ondini (pelle verde mare, pinne), con le stesse animazioni: 48 personaggi in più.

![Popoli](img/popoli.png)

`python3 tools/pixelart/pixellab_import.py` compone il foglio base e scrive `client/assets/override/units.png` (atlante classico) e `units_people.png`: una riga per classe × popolo × squadra, riga `(classe * 5 + popolo) * 2 + squadra`, 11 fotogrammi da 32. Il client usa quest'ultimo quando c'è. Per ogni classe usa:

| Fotogrammi | Da dove |
|---|---|
| riposo, fronte e retro | dalle rotazioni |
| secondo fotogramma del riposo | si ricava abbassando il busto di un pixel |
| azione, KO e camminata | da fotogrammi scelti delle animazioni |
| colore della squadra | il blu saturo dei vestiti va nei colori chiave della squadra; l'acciaio, bluastro ma poco saturo, resta com'è |

**Ritratti** (`tools/pixelart/pixellab/portraits/<popolo>_<classe>.png`): busti 64×64 con lo strumento Pro Flash, tutti nello stile del primo (quello dell'Avanguardia, passato come immagine di stile). Per ora ci sono i dodici ritratti umani; per gli altri popoli il gioco usa quello umano della classe. `python3 tools/pixelart/pixellab_portraits.py` scrive `client/assets/override/portraits.png`: una colonna per classe, riga `popolo * 2 + squadra`; per la squadra 1 il blu (vestiti e sfondo) ruota verso il rosso. Compaiono nella scheda dell'unità in battaglia e nel roster.

![Ritratti](img/ritratti.png)

**Effetti delle abilità** (`tools/pixelart/pixellab/vfx/`): fatti con il laboratorio degli effetti di PixelLab (`vfx_workbench`), che li disegna da parti regolabili senza consumare generazioni: arco della lama, stella d'impatto, esplosione di fuoco, schegge di ghiaccio, saetta, colonna di luce, croci di cura, frecce su e giù, bolle di veleno, energia che converge (risucchio) e la freccia in volo. `python3 tools/pixelart/pixellab_vfx.py` scrive `client/assets/override/vfx.png`: celle 64×64, 8 fotogrammi per riga, centrati sull'origine dell'effetto.

**Fondale e freccia dell'unità attiva** (`tools/pixelart/pixellab/scene/`): cielo al crepuscolo con montagne (256×144, Pro Flash) e freccia dorata (32×32, Pixflux). `python3 tools/pixelart/pixellab_scene.py` li installa in `client/assets/override/` (`backdrop.png` e una copia di `terrain.png` con la nuova freccia).

**Icone degli oggetti** (`tools/pixelart/pixellab/icons/`): una icona 32×32 per ognuna delle 37 basi, dei 16 leggendari e dei 32 consumabili (`use_<codice>`), per stivali e corno (oggetti del catalogo classico) e per il sacchetto del bottino a terra: 88 in tutto.

```bash
PIXELLAB_TOKEN=... python3 tools/pixelart/pixellab_icons.py generate
```

```bash
python3 tools/pixelart/pixellab_icons.py atlas
```

**Terreni** (`tools/pixelart/pixellab/terrain/`): tessere 32×32 fatte con `create_tiles_pro` (sommità e fianchi) e decorazioni fatte con Pixen (alberello, cespuglio, sassi, ciuffo d'erba). `python3 tools/pixelart/pixellab_terrain.py` compone `client/assets/override/terrain3d.png` con celle da 32:
- **sommità:** una tessera base per terreno, con le rotazioni come varianti perché lo stile resti uniforme;
- **bordi:** tolti dalle sommità, così le caselle vicine si fondono;
- **acqua e lava:** tre fotogrammi ottenuti facendo scorrere la tessera;
- **fianchi:** ripuliti dal bordo chiaro.

La mappa 3D ricava la dimensione della cella dall'atlante, quindi funziona sia con le celle da 16 del generatore sia con quelle da 32.

**Interfaccia** (`tools/pixelart/pixellab/ui/`): due pannelli fatti con `create_ui_asset`, uno per la finestra con cornice dorata decorata e uno per il pulsante a bordo fine. `python3 tools/pixelart/pixellab_ui.py` scrive in `client/assets/override/`:
- **finestra:** ritagliata, a 9 parti con margine 18;
- **pulsante:** compattato a 24×24, con margine 9;
- **varianti** (passaggio del mouse, premuto, disattivato, campo di testo): ricavate variando luminosità e saturazione.

**Mappa del mondo** (`tools/pixelart/pixellab/worldmap/`): pergamena 480×272 fatta con lo strumento «Pro» di PixelLab, usando la mappa generata come riferimento per la disposizione delle cinque regioni da ovest a est. È installata come `client/assets/override/worldmap.png`; le tappe della campagna (`server/src/campaign.rs`) sono posizionate sui punti di riferimento del disegno (tempio, oasi, fortezza di lava, Sala dei Custodi). I nomi delle regioni compaiono in targhe in fondo a ogni fascia.

Il primo comando genera solo le icone mancanti; il secondo compone `client/assets/items.png` e l'indice `items.json`. Per rifare un'icona basta cancellarne il PNG e rilanciare `generate`.

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
| `vfx.png` | celle quadrate (32 per il generato, 64 per PixelLab: lato = altezza / 12), fino a 8 fotogrammi per riga. Righe: slash, arrow (proiettile nel fotogramma 0), impact, fire, ice, thunder, holy, heal, buff, debuff, poison, drain. Il client conta da solo i fotogrammi non vuoti |
| `backdrop.png` | fondale dietro la mappa, di qualsiasi misura 16:9 (riempie lo schermo mantenendo le proporzioni) |
| `portraits.png` | ritratti 64×64: colonna = classe, riga = popolo * 2 + squadra |
| `units_people.png` | 11 fotogrammi da 32 per riga, riga = (classe * 5 + popolo) * 2 + squadra |
| `worldmap.png` | 480×270, pergamena della mappa del mondo; le tappe sono in coordinate 0..1 (`server/src/campaign.rs`) |
| `ui_window.png` | 32×32, cornice a 9 parti con margine 8 |
| `ui_button*.png`, `ui_field.png` | 24×24, cornici a 9 parti con margine 6 |
| `fonts/tactica.fnt` e `.png` | font BMFont con cella di 11 pixel (righe 0-1 per gli accenti delle maiuscole, linea di base sotto la riga 8) |

## Audio

Musiche ed effetti sono sintetizzati in codice in `tools/audio/`:
- **`fm.py`:** il motore. Operatori FM in stile OPL3, con forme d'onda del chip, feedback e inviluppi; strumenti orchestrali con coro e vibrato; percussioni sintetiche; riverbero a convoluzione;
- **`music.py`:** sei brani e due fanfare, con melodie e arrangiamenti scritti nel codice. Esce in OGG Vorbis 32 kHz stereo, con loop senza giunture;
- **`sfx.py`:** 66 effetti in WAV mono 32 kHz, con code tagliate sotto i -50 dB.

Ogni file in `client/assets/audio/` si può sostituire con uno dallo stesso nome: `music_*.ogg` (o `.wav`) in loop, `jingle_*.ogg`, `sfx_*.wav`.

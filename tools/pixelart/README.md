# Pixel-art procedurale

Script Python (Pillow) che generano tutta la grafica del client in `client/assets/`. La grafica è originale: niente asset di altri giochi.

```bash
python3 tools/pixelart/build.py
```

```bash
godot --headless --path client --import
```

| File | Contenuto |
|---|---|
| `pixlib.py` | tela a livelli: forme, timbri ASCII, ombreggiatura a smusso con luce dall'alto a sinistra, linee interne tra le parti, contorno colorato |
| `palette.py` | palette del progetto: rampe da 4 toni con ombre fredde e luci calde; squadre, capelli, carnagioni, materiali |
| `units.py` | personaggi chibi delle 6 classi, composti da parti (corpo, abiti, copricapo, arma) |
| `tiles.py` | caselle isometriche 2:1 (rombo 32x16, 8 px per livello) dei 6 terreni, fianchi stratificati, sovrapposizioni (evidenziazione, cursore, freccia, ombra) |
| `build.py` | scrive gli atlanti `terrain.png` e `units.png`, i metadati `units.json` e le cornici dell'interfaccia `ui_*.png` |
| `preview.py` | anteprima ingrandita degli sprite |

## Atlante dei personaggi

- **Fotogrammi:** celle 32x32 con i piedi a (16, 30), in quest'ordine: `front_idle0`, `front_idle1`, `back_idle0`, `back_idle1`, `front_act`, `back_act`, `ko`.
- **Direzioni:** la vista "front" guarda in basso a destra e la "back" in alto a destra. Le altre due direzioni si ottengono specchiando.
- **Righe:** `((classe * 2 + squadra) * 5 + capelli) * 3 + carnagione`. Ci sono 6 classi, 2 colori di squadra, 5 colori di capelli e 3 carnagioni. Il client deriva capelli e carnagione dal nome dell'unità.

## Sostituire la grafica

Il client legge soltanto gli atlanti, quindi per sostituire la grafica con quella di un pixel artist basta rispettare le dimensioni delle celle e l'ordine di fotogrammi e righe.

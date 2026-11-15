//! Griglia con altezze e generazione procedurale da seed.

use std::collections::VecDeque;

use crate::rng::Rng;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

impl Pos {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn dist(self, o: Pos) -> i32 {
        (self.x - o.x).abs() + (self.y - o.y).abs()
    }

    pub fn neighbors(self) -> [Pos; 4] {
        [
            Pos::new(self.x, self.y - 1),
            Pos::new(self.x + 1, self.y),
            Pos::new(self.x, self.y + 1),
            Pos::new(self.x - 1, self.y),
        ]
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dir {
    N,
    E,
    S,
    W,
}

impl Dir {
    /// Direzione dominante da `from` verso `to`.
    pub fn toward(from: Pos, to: Pos) -> Dir {
        let (dx, dy) = (to.x - from.x, to.y - from.y);
        if dx.abs() >= dy.abs() {
            if dx >= 0 { Dir::E } else { Dir::W }
        } else if dy >= 0 {
            Dir::S
        } else {
            Dir::N
        }
    }

    pub fn vec(self) -> (i32, i32) {
        match self {
            Dir::N => (0, -1),
            Dir::E => (1, 0),
            Dir::S => (0, 1),
            Dir::W => (-1, 0),
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Terrain {
    Grass,
    Stone,
    Sand,
    Water,
    Lava,
    Thicket,
}

impl Terrain {
    pub fn move_cost(self) -> i32 {
        match self {
            Terrain::Water | Terrain::Sand | Terrain::Thicket => 2,
            _ => 1,
        }
    }

    pub fn def_bonus(self) -> i32 {
        match self {
            Terrain::Stone => 3,
            _ => 0,
        }
    }

    pub fn evade_bonus(self) -> i32 {
        match self {
            Terrain::Thicket => 15,
            Terrain::Water => -10,
            _ => 0,
        }
    }

    /// Danno a fine turno in % degli HP massimi.
    pub fn end_turn_damage_pct(self) -> i32 {
        match self {
            Terrain::Lava => 10,
            _ => 0,
        }
    }

    /// Effetti del terreno sulle unità, in italiano, ricavati dai valori del motore.
    pub fn effects(self) -> Vec<String> {
        let mut v = Vec::new();
        if self.move_cost() > 1 {
            v.push(format!("costa {} punti movimento", self.move_cost()));
        }
        if self.def_bonus() != 0 {
            v.push(format!("DEF {:+}", self.def_bonus()));
        }
        if self.evade_bonus() != 0 {
            v.push(format!("schivata {:+}%", self.evade_bonus()));
        }
        if self.end_turn_damage_pct() > 0 {
            v.push(format!("-{}% HP a fine turno", self.end_turn_damage_pct()));
        }
        match self {
            Terrain::Water => {
                v.push("fulmine +50%".into());
                v.push("fuoco -50%".into());
            }
            Terrain::Lava => v.push("ghiaccio -50%".into()),
            _ => {}
        }
        v
    }

    pub fn glyph(self) -> char {
        match self {
            Terrain::Grass => '.',
            Terrain::Stone => '#',
            Terrain::Sand => ':',
            Terrain::Water => '~',
            Terrain::Lava => '^',
            Terrain::Thicket => '"',
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Terrain::Grass => "erba",
            Terrain::Stone => "roccia",
            Terrain::Sand => "sabbia",
            Terrain::Water => "acqua",
            Terrain::Lava => "lava",
            Terrain::Thicket => "boscaglia",
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Biome {
    Meadow,
    Desert,
    Volcano,
    Marsh,
    Highlands,
}

impl Biome {
    pub const ALL: [Biome; 5] = [
        Biome::Meadow,
        Biome::Desert,
        Biome::Volcano,
        Biome::Marsh,
        Biome::Highlands,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Biome::Meadow => "Prateria",
            Biome::Desert => "Deserto",
            Biome::Volcano => "Vulcano",
            Biome::Marsh => "Palude",
            Biome::Highlands => "Altopiano",
        }
    }

    fn pick_terrain(self, h: i32, max_h: i32, moisture: i32) -> Terrain {
        let low = h * 3 <= max_h;
        let high = h * 3 >= max_h * 2;
        match self {
            Biome::Meadow => {
                if moisture > 80 && low {
                    Terrain::Water
                } else if moisture > 64 {
                    Terrain::Thicket
                } else if high {
                    Terrain::Stone
                } else {
                    Terrain::Grass
                }
            }
            Biome::Desert => {
                if moisture > 88 && low {
                    Terrain::Water
                } else if high || moisture < 12 {
                    Terrain::Stone
                } else {
                    Terrain::Sand
                }
            }
            Biome::Volcano => {
                if moisture > 72 && low {
                    Terrain::Lava
                } else if moisture < 28 {
                    Terrain::Sand
                } else {
                    Terrain::Stone
                }
            }
            Biome::Marsh => {
                if moisture > 52 && low {
                    Terrain::Water
                } else if moisture > 40 {
                    Terrain::Thicket
                } else {
                    Terrain::Grass
                }
            }
            Biome::Highlands => {
                if high {
                    Terrain::Stone
                } else if moisture > 72 {
                    Terrain::Thicket
                } else {
                    Terrain::Grass
                }
            }
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapParams {
    pub width: i32,
    pub height: i32,
    pub biome: Biome,
    /// Altezza massima del terreno (livelli).
    pub max_height: i32,
    /// 0..=100: quanto il rilievo è frastagliato.
    pub roughness: i32,
    /// Posizioni di schieramento per squadra.
    pub spawns_per_team: usize,
}

impl MapParams {
    pub fn random(rng: &mut Rng) -> Self {
        let biome = Biome::ALL[rng.below(Biome::ALL.len() as u32) as usize];
        let extra = if biome == Biome::Highlands { 3 } else { 0 };
        MapParams {
            width: rng.range(14, 20),
            height: rng.range(12, 17),
            biome,
            max_height: rng.range(4, 8) + extra,
            roughness: rng.range(10, 60),
            spawns_per_team: 6,
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tile {
    pub height: i32,
    pub terrain: Terrain,
}

impl Tile {
    pub fn standable(self) -> bool {
        !matches!(self.terrain, Terrain::Water | Terrain::Lava)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Map {
    pub width: i32,
    pub height: i32,
    pub biome: Biome,
    tiles: Vec<Tile>,
    pub spawns: [Vec<Pos>; 2],
}

/// Salto usato per verificare che le squadre possano raggiungersi.
const CONNECTIVITY_JUMP: i32 = 2;

impl Map {
    pub fn generate(params: &MapParams, rng: &mut Rng) -> Map {
        for _ in 0..64 {
            let mut attempt = rng.fork();
            if let Some(map) = Self::try_generate(params, &mut attempt) {
                return map;
            }
        }
        Self::flat(params)
    }

    pub fn in_bounds(&self, p: Pos) -> bool {
        p.x >= 0 && p.y >= 0 && p.x < self.width && p.y < self.height
    }

    pub fn tile(&self, p: Pos) -> Option<Tile> {
        self.in_bounds(p)
            .then(|| self.tiles[(p.y * self.width + p.x) as usize])
    }

    pub fn positions(&self) -> impl Iterator<Item = Pos> + '_ {
        (0..self.height).flat_map(move |y| (0..self.width).map(move |x| Pos::new(x, y)))
    }

    /// Mappa piatta di riserva se la generazione fallisce ripetutamente.
    pub fn flat(params: &MapParams) -> Map {
        let tiles = vec![
            Tile {
                height: 0,
                terrain: Terrain::Grass,
            };
            (params.width * params.height) as usize
        ];
        let n = params.spawns_per_team.min(params.height as usize * 2);
        let side = |team: i32| -> Vec<Pos> {
            (0..n as i32)
                .map(|i| {
                    let dx = i % 2;
                    let x = if team == 0 { dx } else { params.width - 1 - dx };
                    Pos::new(x, i / 2)
                })
                .collect()
        };
        Map {
            width: params.width,
            height: params.height,
            biome: params.biome,
            tiles,
            spawns: [side(0), side(1)],
        }
    }

    fn try_generate(params: &MapParams, rng: &mut Rng) -> Option<Map> {
        let (w, h) = (params.width, params.height);
        let max_h = params.max_height.max(1);

        let landform = Landform::pick(params.biome, rng);
        let elev = shaped_elevation(w, h, max_h, landform, rng);
        let moist = fractal_noise(w, h, 99 * 16, rng);
        let jitter = params.roughness.clamp(0, 100) * max_h * 16 / 260;

        let mut tiles = Vec::with_capacity((w * h) as usize);
        for i in 0..(w * h) as usize {
            let j = if jitter > 0 {
                rng.range(-jitter, jitter)
            } else {
                0
            };
            let mut height = ((elev[i] + j) / 16).clamp(0, max_h);
            if landform == Landform::Terraces {
                // Gradoni larghi: altezze pari, salti di due livelli.
                height = (height / 2 * 2).min(max_h);
            }
            let terrain = params.biome.pick_terrain(height, max_h, moist[i] / 16);
            tiles.push(Tile { height, terrain });
        }
        landform.carve(&mut tiles, w, h, max_h, params.biome, rng);
        scatter_features(&mut tiles, w, h, max_h, params.biome, rng);
        // Le superfici liquide sono piatte e al livello più basso del vicinato.
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                if matches!(tiles[i].terrain, Terrain::Water | Terrain::Lava) {
                    tiles[i].height = 0;
                }
            }
        }

        let mut map = Map {
            width: w,
            height: h,
            biome: params.biome,
            tiles,
            spawns: [Vec::new(), Vec::new()],
        };

        for team in 0..2 {
            let xs = if team == 0 { 0..3 } else { w - 3..w };
            let mut cands: Vec<Pos> = (0..h)
                .flat_map(|y| xs.clone().map(move |x| Pos::new(x, y)))
                .filter(|&p| map.tile(p).is_some_and(Tile::standable))
                .collect();
            if cands.len() < params.spawns_per_team {
                return None;
            }
            rng.shuffle(&mut cands);
            cands.truncate(params.spawns_per_team);
            cands.sort();
            map.spawns[team] = cands;
        }

        map.spawns_connected().then_some(map)
    }

    /// Tutti gli schieramenti sono collegati a piedi (salto 2, acqua e lava attraversabili)?
    pub fn spawns_connected(&self) -> bool {
        let Some(&start) = self.spawns[0].first() else {
            return false;
        };
        let mut seen = vec![false; self.tiles.len()];
        let idx = |p: Pos| (p.y * self.width + p.x) as usize;
        let mut q = VecDeque::from([start]);
        seen[idx(start)] = true;
        while let Some(p) = q.pop_front() {
            let hp = self.tile(p).unwrap().height;
            for n in p.neighbors() {
                if let Some(t) = self.tile(n)
                    && !seen[idx(n)]
                    && (t.height - hp).abs() <= CONNECTIVITY_JUMP
                {
                    seen[idx(n)] = true;
                    q.push_back(n);
                }
            }
        }
        self.spawns.iter().flatten().all(|&p| seen[idx(p)])
    }

    /// Rappresentazione testuale: terreno e altezza per casella.
    pub fn render(&self, marks: &dyn Fn(Pos) -> Option<char>) -> String {
        let mut s = String::new();
        s.push_str("    ");
        for x in 0..self.width {
            s.push_str(&format!("{x:>3}"));
        }
        s.push('\n');
        for y in 0..self.height {
            s.push_str(&format!("{y:>3} "));
            for x in 0..self.width {
                let p = Pos::new(x, y);
                let t = self.tile(p).unwrap();
                let c = marks(p).unwrap_or(t.terrain.glyph());
                s.push_str(&format!(" {c}{}", t.height));
            }
            s.push('\n');
        }
        s
    }
}

/// Forma del terreno di una mappa, estratta secondo il bioma.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Landform {
    /// Colline morbide.
    Rolling,
    /// Vallata fra due crinali, aperta da una squadra all'altra.
    Valley,
    /// Colle o mesa al centro.
    Hill,
    /// Gola stretta con pareti ripide.
    Canyon,
    /// Gradoni larghi.
    Terraces,
    /// Fiume (o colata) che attraversa la mappa, con guadi.
    River,
    /// Laghetti e stagni sparsi.
    Lakes,
    /// Pinnacoli di roccia su un terreno mosso.
    Pillars,
}

impl Landform {
    pub fn name(self) -> &'static str {
        match self {
            Landform::Rolling => "colline",
            Landform::Valley => "vallata",
            Landform::Hill => "colle",
            Landform::Canyon => "gola",
            Landform::Terraces => "gradoni",
            Landform::River => "fiume",
            Landform::Lakes => "laghi",
            Landform::Pillars => "pinnacoli",
        }
    }

    fn pick(biome: Biome, rng: &mut Rng) -> Landform {
        use Landform::*;
        let pool: &[(Landform, u32)] = match biome {
            Biome::Meadow => &[
                (Rolling, 3),
                (Valley, 3),
                (Hill, 3),
                (River, 4),
                (Lakes, 2),
                (Terraces, 1),
            ],
            Biome::Desert => &[
                (Rolling, 2),
                (Canyon, 4),
                (Pillars, 4),
                (Hill, 2),
                (Lakes, 1),
            ],
            Biome::Volcano => &[
                (Hill, 3),
                (River, 4),
                (Canyon, 2),
                (Pillars, 2),
                (Rolling, 1),
            ],
            Biome::Marsh => &[(Lakes, 5), (River, 3), (Rolling, 2), (Valley, 1)],
            Biome::Highlands => &[
                (Terraces, 4),
                (Valley, 3),
                (Canyon, 2),
                (Hill, 2),
                (Pillars, 2),
            ],
        };
        let total: u32 = pool.iter().map(|p| p.1).sum();
        let mut roll = rng.below(total);
        for &(l, wt) in pool {
            if roll < wt {
                return l;
            }
            roll -= wt;
        }
        Rolling
    }

    /// Ritocchi dopo l'assegnazione dei terreni: fiumi, laghi, pinnacoli.
    fn carve(self, tiles: &mut [Tile], w: i32, h: i32, max_h: i32, biome: Biome, rng: &mut Rng) {
        let liquid = if biome == Biome::Volcano {
            Terrain::Lava
        } else {
            Terrain::Water
        };
        let at = |x: i32, y: i32| (y * w + x) as usize;
        match self {
            Landform::River => {
                // Scorre da nord a sud fra i due schieramenti, serpeggiando.
                let mut x = w / 2 + rng.range(-1, 1);
                let fords: Vec<i32> = (0..2).map(|_| rng.range(1, h - 2)).collect();
                for y in 0..h {
                    x = (x + rng.range(-1, 1)).clamp(4, w - 5);
                    let width = if rng.percent(35) { 2 } else { 1 };
                    for dx in 0..width {
                        let i = at(x + dx, y);
                        if fords.contains(&y) {
                            // Guado: basso e attraversabile, sabbia o roccia.
                            tiles[i] = Tile {
                                height: 0,
                                terrain: if liquid == Terrain::Lava {
                                    Terrain::Stone
                                } else {
                                    Terrain::Sand
                                },
                            };
                        } else {
                            tiles[i] = Tile {
                                height: 0,
                                terrain: liquid,
                            };
                        }
                    }
                    // Sponde più basse accanto al fiume.
                    for bx in [x - 1, x + width] {
                        let i = at(bx, y);
                        tiles[i].height = tiles[i].height.min(1);
                    }
                }
            }
            Landform::Lakes => {
                let n = rng.range(2, 4);
                for _ in 0..n {
                    let (cx, cy) = (rng.range(3, w - 4), rng.range(1, h - 2));
                    let r = rng.range(1, 2);
                    for y in (cy - r).max(0)..=(cy + r).min(h - 1) {
                        for x in (cx - r).max(0)..=(cx + r).min(w - 1) {
                            if (x - cx).abs() + (y - cy).abs() <= r + 1 && rng.percent(85) {
                                tiles[at(x, y)] = Tile {
                                    height: 0,
                                    terrain: liquid,
                                };
                            }
                        }
                    }
                }
            }
            Landform::Pillars => {
                let n = rng.range(4, 9);
                for _ in 0..n {
                    let (x, y) = (rng.range(3, w - 4), rng.range(0, h - 1));
                    let i = at(x, y);
                    tiles[i] = Tile {
                        height: (tiles[i].height + rng.range(2, 4)).min(max_h + 2),
                        terrain: Terrain::Stone,
                    };
                }
            }
            _ => {}
        }
    }
}

/// Dettagli naturali sparsi: affioramenti di roccia e macchie di vegetazione.
fn scatter_features(tiles: &mut [Tile], w: i32, h: i32, max_h: i32, biome: Biome, rng: &mut Rng) {
    let area = w * h;
    let rocks = area / 40 + rng.range(0, 3);
    for _ in 0..rocks {
        let i = rng.below(area as u32) as usize;
        if tiles[i].standable() {
            tiles[i].terrain = Terrain::Stone;
            if rng.percent(50) {
                tiles[i].height = (tiles[i].height + 1).min(max_h + 1);
            }
        }
    }
    let veg = match biome {
        Biome::Meadow | Biome::Marsh | Biome::Highlands => area / 30,
        _ => 0,
    };
    for _ in 0..veg {
        let (cx, cy) = (rng.range(0, w - 1), rng.range(0, h - 1));
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (-1, 0), (0, -1)] {
            let (x, y) = (cx + dx, cy + dy);
            if x >= 0 && y >= 0 && x < w && y < h && rng.percent(70) {
                let i = (y * w + x) as usize;
                if tiles[i].terrain == Terrain::Grass {
                    tiles[i].terrain = Terrain::Thicket;
                }
            }
        }
    }
}

/// Rumore su tre scale (forme ampie, medie e piccole), valori `0..=max`.
fn fractal_noise(w: i32, h: i32, max: i32, rng: &mut Rng) -> Vec<i32> {
    let a = value_noise(w, h, 6, max, rng);
    let b = value_noise(w, h, 3, max, rng);
    let c = value_noise(w, h, 2, max, rng);
    (0..a.len())
        .map(|i| (a[i] * 55 + b[i] * 30 + c[i] * 15) / 100)
        .collect()
}

/// Rilievo modellato secondo la forma del terreno (virgola fissa ×16).
fn shaped_elevation(w: i32, h: i32, max_h: i32, form: Landform, rng: &mut Rng) -> Vec<i32> {
    let max = max_h * 16;
    let base = fractal_noise(w, h, max, rng);
    let mut out = Vec::with_capacity(base.len());
    for y in 0..h {
        for x in 0..w {
            let n = base[(y * w + x) as usize];
            // Distanza dal centro normalizzata a 0..=100.
            let dy = (2 * y - (h - 1)).abs() * 100 / h.max(1);
            let dx = (2 * x - (w - 1)).abs() * 100 / w.max(1);
            let d = ((dx * dx + dy * dy) as f64).sqrt().min(100.0) as i32;
            let v = match form {
                Landform::Valley => n * 50 / 100 + max * dy / 100 * 70 / 100,
                Landform::Canyon => {
                    let wall = if dy > 30 {
                        max * 85 / 100
                    } else {
                        max * 5 / 100
                    };
                    n * 30 / 100 + wall
                }
                Landform::Hill => n * 45 / 100 + max * (100 - d) / 100 * 75 / 100,
                Landform::Terraces => n * 60 / 100 + max * x / w.max(1) * 50 / 100,
                Landform::River | Landform::Lakes => n * 85 / 100,
                Landform::Rolling | Landform::Pillars => n,
            };
            out.push(v.clamp(0, max));
        }
    }
    out
}

/// Rumore a reticolo con interpolazione bilineare, in virgola fissa (valori `0..=max`).
fn value_noise(w: i32, h: i32, cell: i32, max: i32, rng: &mut Rng) -> Vec<i32> {
    let gw = w / cell + 2;
    let gh = h / cell + 2;
    let lat: Vec<i32> = (0..gw * gh).map(|_| rng.range(0, max)).collect();
    let at = |gx: i32, gy: i32| lat[(gy * gw + gx) as usize];
    let mut out = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let (gx, gy, fx, fy) = (x / cell, y / cell, x % cell, y % cell);
            let top = at(gx, gy) * (cell - fx) + at(gx + 1, gy) * fx;
            let bot = at(gx, gy + 1) * (cell - fx) + at(gx + 1, gy + 1) * fx;
            out.push((top * (cell - fy) + bot * fy) / (cell * cell));
        }
    }
    out
}

//! Griglia con altezze e generazione procedurale da seed.

use std::collections::VecDeque;

use crate::rng::Rng;

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
            Terrain::Water | Terrain::Sand => 2,
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
            width: rng.range(10, 14),
            height: rng.range(9, 12),
            biome,
            max_height: rng.range(3, 6) + extra,
            roughness: rng.range(10, 60),
            spawns_per_team: 6,
        }
    }
}

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

        let elev = value_noise(w, h, 4, max_h * 16, rng);
        let moist = value_noise(w, h, 3, 99 * 16, rng);
        let jitter = params.roughness.clamp(0, 100) * max_h * 16 / 200;

        let mut tiles = Vec::with_capacity((w * h) as usize);
        for i in 0..(w * h) as usize {
            let j = if jitter > 0 {
                rng.range(-jitter, jitter)
            } else {
                0
            };
            let height = ((elev[i] + j) / 16).clamp(0, max_h);
            let terrain = params.biome.pick_terrain(height, max_h, moist[i] / 16);
            tiles.push(Tile { height, terrain });
        }
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

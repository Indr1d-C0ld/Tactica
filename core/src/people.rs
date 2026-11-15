//! Popoli: originali di Velmara. Ognuno modifica statistiche, movimento e il
//! rapporto con certi terreni. Si sceglie alla recluta e non cambia.

use crate::class::Stats;
use crate::map::Terrain;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum People {
    /// Equilibrati e adattabili.
    #[default]
    Human,
    /// Gente dei boschi: agili nella vegetazione, fragili in difesa.
    Sylvan,
    /// Corpi di pietra viva: lenti ma robusti, la lava non li ferisce.
    Stonekin,
    /// Con ali piumate: saltano altissimo e passano sull'acqua.
    Winged,
    /// Popolo delle acque: a loro agio nell'acqua, impacciati nella sabbia.
    Undine,
}

impl People {
    pub const ALL: [People; 5] = [
        People::Human,
        People::Sylvan,
        People::Stonekin,
        People::Winged,
        People::Undine,
    ];

    pub fn key(self) -> &'static str {
        match self {
            People::Human => "human",
            People::Sylvan => "sylvan",
            People::Stonekin => "stonekin",
            People::Winged => "winged",
            People::Undine => "undine",
        }
    }

    pub fn from_key(k: &str) -> Option<People> {
        People::ALL.into_iter().find(|p| p.key() == k)
    }

    pub fn name(self) -> &'static str {
        match self {
            People::Human => "Umano",
            People::Sylvan => "Silvano",
            People::Stonekin => "Roccioso",
            People::Winged => "Alato",
            People::Undine => "Ondino",
        }
    }

    /// Descrizione dei tratti, per l'interfaccia.
    pub fn traits(self) -> &'static str {
        match self {
            People::Human => "Equilibrati: ATK, DEF, MAG e RES +1.",
            People::Sylvan => "MOV +1; la boscaglia costa 1 e dà schivata +10; DEF −2, HP −5%.",
            People::Stonekin => {
                "MOV −1, salto −1; HP +15%, DEF +3; la lava non li brucia e costa 1."
            }
            People::Winged => "Salto +2; l'acqua costa 1; schivata +5; HP −5%.",
            People::Undine => "L'acqua costa 1 e dà schivata +15; la sabbia costa 3; MAG +2.",
        }
    }

    /// Statistiche modificate dal popolo.
    pub fn apply(self, mut s: Stats) -> Stats {
        match self {
            People::Human => {
                s.atk += 1;
                s.def += 1;
                s.mag += 1;
                s.res += 1;
            }
            People::Sylvan => {
                s.mov += 1;
                s.def -= 2;
                s.max_hp = s.max_hp * 95 / 100;
            }
            People::Stonekin => {
                s.mov -= 1;
                s.jump -= 1;
                s.max_hp = s.max_hp * 115 / 100;
                s.def += 3;
            }
            People::Winged => {
                s.jump += 2;
                s.max_hp = s.max_hp * 95 / 100;
                s.evade += 5;
            }
            People::Undine => s.mag += 2,
        }
        s
    }

    /// Costo di movimento del terreno per questo popolo (`None` = quello normale).
    pub fn terrain_cost(self, t: Terrain) -> Option<i32> {
        match (self, t) {
            (People::Sylvan, Terrain::Thicket) => Some(1),
            (People::Stonekin, Terrain::Lava) => Some(1),
            (People::Winged | People::Undine, Terrain::Water) => Some(1),
            (People::Undine, Terrain::Sand) => Some(3),
            _ => None,
        }
    }

    /// Schivata in più su questo terreno.
    pub fn terrain_evade(self, t: Terrain) -> i32 {
        match (self, t) {
            (People::Sylvan, Terrain::Thicket) => 10,
            (People::Undine, Terrain::Water) => 15,
            _ => 0,
        }
    }

    /// Immune al danno della lava?
    pub fn lava_proof(self) -> bool {
        self == People::Stonekin
    }
}

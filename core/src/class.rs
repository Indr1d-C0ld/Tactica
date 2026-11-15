//! Classi originali, crescita delle statistiche e requisiti di sblocco.

use crate::ability::AbilityId;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Stats {
    pub max_hp: i32,
    pub max_mp: i32,
    pub atk: i32,
    pub def: i32,
    pub mag: i32,
    pub res: i32,
    pub spd: i32,
    pub mov: i32,
    pub jump: i32,
    pub evade: i32,
}

impl Stats {
    pub const ZERO: Stats = Stats {
        max_hp: 0,
        max_mp: 0,
        atk: 0,
        def: 0,
        mag: 0,
        res: 0,
        spd: 0,
        mov: 0,
        jump: 0,
        evade: 0,
    };

    pub fn plus(self, o: Stats) -> Stats {
        Stats {
            max_hp: self.max_hp + o.max_hp,
            max_mp: self.max_mp + o.max_mp,
            atk: self.atk + o.atk,
            def: self.def + o.def,
            mag: self.mag + o.mag,
            res: self.res + o.res,
            spd: self.spd + o.spd,
            mov: self.mov + o.mov,
            jump: self.jump + o.jump,
            evade: self.evade + o.evade,
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Class {
    Vanguard,
    Scout,
    Arcanist,
    Mender,
    Runeblade,
    Warden,
}

pub const MAX_LEVEL: i32 = 50;

/// Condizioni per accedere a una classe avanzata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Requirement {
    pub level: i32,
    /// Per ogni classe, quante abilità della sua scuola vanno padroneggiate.
    pub mastered: &'static [(Class, usize)],
}

impl Class {
    pub const ALL: [Class; 6] = [
        Class::Vanguard,
        Class::Scout,
        Class::Arcanist,
        Class::Mender,
        Class::Runeblade,
        Class::Warden,
    ];

    /// Chiave stabile per database e protocollo.
    pub fn key(self) -> &'static str {
        match self {
            Class::Vanguard => "vanguard",
            Class::Scout => "scout",
            Class::Arcanist => "arcanist",
            Class::Mender => "mender",
            Class::Runeblade => "runeblade",
            Class::Warden => "warden",
        }
    }

    pub fn from_key(key: &str) -> Option<Class> {
        Class::ALL.into_iter().find(|c| c.key() == key)
    }

    pub fn name(self) -> &'static str {
        match self {
            Class::Vanguard => "Avanguardia",
            Class::Scout => "Esploratore",
            Class::Arcanist => "Arcanista",
            Class::Mender => "Guaritore",
            Class::Runeblade => "Spadaruna",
            Class::Warden => "Custode",
        }
    }

    /// Abilità sempre disponibili per la classe (oltre a Colpo, universale).
    pub fn innate(self) -> &'static [AbilityId] {
        use AbilityId::*;
        match self {
            Class::Vanguard => &[HeavyBlow],
            Class::Scout => &[Shot],
            Class::Arcanist => &[Flame],
            Class::Mender => &[Mend],
            Class::Runeblade => &[RuneEdge],
            Class::Warden => &[Rebuke],
        }
    }

    /// Abilità della scuola, da imparare tramite equipaggiamento.
    pub fn learnable(self) -> &'static [AbilityId] {
        use AbilityId::*;
        match self {
            Class::Vanguard => &[Sweep, Charge],
            Class::Scout => &[AimedShot, Volley],
            Class::Arcanist => &[Frost, Bolt, Inferno],
            Class::Mender => &[Radiance, GreatMend],
            Class::Runeblade => &[DrainBlade, FrostEdge],
            Class::Warden => &[Sanctuary, Judgment],
        }
    }

    /// `None` = classe base, sempre disponibile.
    pub fn requirement(self) -> Option<Requirement> {
        match self {
            Class::Runeblade => Some(Requirement {
                level: 5,
                mastered: &[(Class::Vanguard, 1), (Class::Arcanist, 1)],
            }),
            Class::Warden => Some(Requirement {
                level: 5,
                mastered: &[(Class::Mender, 1), (Class::Vanguard, 1)],
            }),
            _ => None,
        }
    }

    pub fn is_base(self) -> bool {
        self.requirement().is_none()
    }

    /// Un'unità con questo livello e queste abilità padroneggiate può usare la classe?
    pub fn unlocked_for(self, level: i32, mastered: &[AbilityId]) -> bool {
        let Some(req) = self.requirement() else {
            return true;
        };
        level >= req.level
            && req.mastered.iter().all(|&(school, n)| {
                school
                    .learnable()
                    .iter()
                    .filter(|a| mastered.contains(a))
                    .count()
                    >= n
            })
    }

    pub fn requirement_text(self) -> String {
        match self.requirement() {
            None => "classe base".to_string(),
            Some(r) => {
                let parts: Vec<String> = r
                    .mastered
                    .iter()
                    .map(|(c, n)| format!("{n} abilità di {} padroneggiate", c.name()))
                    .collect();
                format!("livello {}, {}", r.level, parts.join(", "))
            }
        }
    }

    fn base(self) -> Stats {
        let s = |max_hp, max_mp, atk, def, mag, res, spd, mov, jump, evade| Stats {
            max_hp,
            max_mp,
            atk,
            def,
            mag,
            res,
            spd,
            mov,
            jump,
            evade,
        };
        match self {
            Class::Vanguard => s(110, 12, 16, 12, 6, 8, 9, 4, 2, 5),
            Class::Scout => s(85, 16, 14, 8, 7, 8, 12, 5, 3, 15),
            Class::Arcanist => s(70, 40, 7, 6, 17, 12, 10, 3, 2, 5),
            Class::Mender => s(75, 36, 8, 7, 14, 13, 10, 4, 2, 8),
            Class::Runeblade => s(95, 24, 15, 10, 13, 10, 10, 4, 2, 8),
            Class::Warden => s(120, 26, 11, 15, 10, 14, 8, 3, 2, 4),
        }
    }

    /// Crescita ogni 10 livelli (virgola fissa: si divide per 10).
    fn growth(self) -> Stats {
        let g = |max_hp, max_mp, atk, def, mag, res, spd| Stats {
            max_hp,
            max_mp,
            atk,
            def,
            mag,
            res,
            spd,
            ..Stats::default()
        };
        match self {
            Class::Vanguard => g(90, 10, 14, 12, 4, 7, 5),
            Class::Scout => g(70, 12, 13, 7, 5, 7, 8),
            Class::Arcanist => g(55, 30, 4, 5, 15, 11, 6),
            Class::Mender => g(60, 28, 5, 6, 12, 12, 6),
            Class::Runeblade => g(80, 18, 12, 9, 11, 9, 6),
            Class::Warden => g(95, 16, 8, 14, 8, 13, 4),
        }
    }

    pub fn stats_at(self, level: i32) -> Stats {
        let (b, g) = (self.base(), self.growth());
        let l = level.clamp(1, MAX_LEVEL) - 1;
        let f = |base: i32, gr: i32| base + gr * l / 10;
        Stats {
            max_hp: f(b.max_hp, g.max_hp),
            max_mp: f(b.max_mp, g.max_mp),
            atk: f(b.atk, g.atk),
            def: f(b.def, g.def),
            mag: f(b.mag, g.mag),
            res: f(b.res, g.res),
            spd: f(b.spd, g.spd),
            mov: b.mov,
            jump: b.jump,
            evade: b.evade,
        }
    }
}

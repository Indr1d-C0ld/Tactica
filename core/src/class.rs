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
    Shadowblade,
    Dragoon,
    Bard,
    Alchemist,
    Geomancer,
    Summoner,
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
    pub const ALL: [Class; 12] = [
        Class::Vanguard,
        Class::Scout,
        Class::Arcanist,
        Class::Mender,
        Class::Runeblade,
        Class::Warden,
        Class::Shadowblade,
        Class::Dragoon,
        Class::Bard,
        Class::Alchemist,
        Class::Geomancer,
        Class::Summoner,
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
            Class::Shadowblade => "shadowblade",
            Class::Dragoon => "dragoon",
            Class::Bard => "bard",
            Class::Alchemist => "alchemist",
            Class::Geomancer => "geomancer",
            Class::Summoner => "summoner",
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
            Class::Shadowblade => "Lama d'ombra",
            Class::Dragoon => "Dragone",
            Class::Bard => "Bardo",
            Class::Alchemist => "Alchimista",
            Class::Geomancer => "Geomante",
            Class::Summoner => "Invocatore",
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
            Class::Shadowblade => &[Backstab],
            Class::Dragoon => &[Leap],
            Class::Bard => &[Ballad],
            Class::Alchemist => &[Lob],
            Class::Geomancer => &[Landslide],
            Class::Summoner => &[Spark],
        }
    }

    /// Abilità della scuola, da imparare tramite equipaggiamento.
    pub fn learnable(self) -> &'static [AbilityId] {
        use AbilityId::*;
        match self {
            Class::Vanguard => &[Sweep, Charge, Rally, LegSweep, Bulwark, Cleave],
            Class::Scout => &[
                AimedShot,
                Volley,
                VenomDart,
                PiercingArrow,
                HushArrow,
                BrambleSnare,
            ],
            Class::Arcanist => &[
                Frost,
                Bolt,
                Inferno,
                Torpor,
                Blizzard,
                Thunderstorm,
                ArcaneHush,
                Meteor,
            ],
            Class::Mender => &[
                Radiance, GreatMend, Aegis, Sprout, Purify, Prayer, HolyRay, Return,
            ],
            Class::Runeblade => &[DrainBlade, FrostEdge, FlameEdge, StormEdge, RuneSeal],
            Class::Warden => &[Sanctuary, Judgment, Chains, ShieldWall, LightHammer, Vow],
            Class::Shadowblade => &[SleepBlade, VenomFang, BladeDance, ShadowVeil],
            Class::Dragoon => &[DiveStrike, DragonLance, Roar, DragonWings],
            Class::Bard => &[WarSong, Lullaby, Dirge, Hymn],
            Class::Alchemist => &[Blastmix, ToxicFumes, Panacea, Tonic],
            Class::Geomancer => &[Quicksand, Earthquake, Stoneskin, Sap],
            Class::Summoner => &[FireSpirit, FrostSpirit, ThunderSpirit, DawnSpirit],
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
            Class::Shadowblade => Some(Requirement {
                level: 8,
                mastered: &[(Class::Scout, 2), (Class::Runeblade, 1)],
            }),
            Class::Dragoon => Some(Requirement {
                level: 8,
                mastered: &[(Class::Vanguard, 2), (Class::Warden, 1)],
            }),
            Class::Bard => Some(Requirement {
                level: 6,
                mastered: &[(Class::Mender, 2), (Class::Scout, 1)],
            }),
            Class::Alchemist => Some(Requirement {
                level: 6,
                mastered: &[(Class::Mender, 1), (Class::Arcanist, 1)],
            }),
            Class::Geomancer => Some(Requirement {
                level: 8,
                mastered: &[(Class::Warden, 1), (Class::Arcanist, 2)],
            }),
            Class::Summoner => Some(Requirement {
                level: 10,
                mastered: &[(Class::Arcanist, 3), (Class::Mender, 1)],
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
            Class::Warden => s(125, 26, 13, 15, 12, 14, 10, 3, 2, 4),
            Class::Shadowblade => s(88, 20, 15, 8, 8, 9, 13, 5, 3, 18),
            Class::Dragoon => s(112, 14, 17, 13, 5, 8, 9, 4, 5, 6),
            Class::Bard => s(80, 34, 9, 8, 12, 12, 11, 4, 2, 10),
            Class::Alchemist => s(88, 30, 11, 9, 13, 11, 10, 4, 2, 8),
            Class::Geomancer => s(98, 30, 12, 11, 15, 12, 9, 4, 3, 7),
            Class::Summoner => s(65, 52, 6, 6, 19, 14, 9, 3, 2, 5),
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
            Class::Warden => g(98, 16, 10, 14, 10, 13, 5),
            Class::Shadowblade => g(72, 14, 13, 7, 6, 8, 9),
            Class::Dragoon => g(92, 10, 14, 12, 4, 7, 5),
            Class::Bard => g(65, 26, 7, 7, 10, 11, 7),
            Class::Alchemist => g(68, 22, 8, 8, 10, 10, 6),
            Class::Geomancer => g(80, 22, 9, 10, 14, 11, 5),
            Class::Summoner => g(52, 36, 4, 5, 17, 13, 5),
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

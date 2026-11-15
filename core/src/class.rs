//! Classi originali e crescita delle statistiche.

use crate::ability::AbilityId;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Class {
    Vanguard,
    Scout,
    Arcanist,
    Mender,
}

pub const MAX_LEVEL: i32 = 50;

impl Class {
    pub const ALL: [Class; 4] = [
        Class::Vanguard,
        Class::Scout,
        Class::Arcanist,
        Class::Mender,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Class::Vanguard => "Avanguardia",
            Class::Scout => "Esploratore",
            Class::Arcanist => "Arcanista",
            Class::Mender => "Guaritore",
        }
    }

    pub fn abilities(self) -> &'static [AbilityId] {
        use AbilityId::*;
        match self {
            Class::Vanguard => &[Strike, HeavyBlow],
            Class::Scout => &[Strike, Shot, AimedShot],
            Class::Arcanist => &[Strike, Flame, Frost, Bolt],
            Class::Mender => &[Strike, Mend, Radiance],
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

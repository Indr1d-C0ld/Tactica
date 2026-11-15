//! Unità in battaglia e contatori statistici (base per le statistiche persistenti).

use crate::class::{Class, MAX_LEVEL, Stats};
use crate::map::{Dir, Pos};

pub type UnitId = u32;

pub const XP_PER_LEVEL: i32 = 100;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Condition {
    Active,
    Defeated,
    Expelled,
}

/// Contatori per unità: alimentano le statistiche personali e globali sul server.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    pub damage_dealt: i32,
    pub damage_taken: i32,
    pub healing_done: i32,
    pub kills: i32,
    pub actions: i32,
    pub misses: i32,
    pub violations: i32,
    pub xp_gained: i32,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitSetup {
    pub name: String,
    pub team: u8,
    pub class: Class,
    pub level: i32,
    /// Esperienza accumulata verso il prossimo livello (progressione persistente).
    #[cfg_attr(feature = "serde", serde(default))]
    pub xp: i32,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unit {
    pub id: UnitId,
    pub name: String,
    pub team: u8,
    pub class: Class,
    pub level: i32,
    pub xp: i32,
    pub stats: Stats,
    pub hp: i32,
    pub mp: i32,
    pub pos: Pos,
    pub facing: Dir,
    pub ct: i32,
    pub condition: Condition,
    pub suspended: i32,
    pub penalties: i32,
    pub tally: Tally,
}

impl Unit {
    pub fn new(id: UnitId, setup: &UnitSetup, pos: Pos, facing: Dir) -> Self {
        let level = setup.level.clamp(1, MAX_LEVEL);
        let stats = setup.class.stats_at(level);
        Unit {
            id,
            name: setup.name.clone(),
            team: setup.team,
            class: setup.class,
            level,
            xp: setup.xp.clamp(0, XP_PER_LEVEL - 1),
            stats,
            hp: stats.max_hp,
            mp: stats.max_mp,
            pos,
            facing,
            ct: 0,
            condition: Condition::Active,
            suspended: 0,
            penalties: 0,
            tally: Tally::default(),
        }
    }

    pub fn is_active(&self) -> bool {
        self.condition == Condition::Active
    }

    /// Aggiunge esperienza; restituisce i livelli guadagnati.
    pub fn gain_xp(&mut self, xp: i32) -> i32 {
        if self.level >= MAX_LEVEL {
            return 0;
        }
        self.xp += xp;
        self.tally.xp_gained += xp;
        let mut gained = 0;
        while self.xp >= XP_PER_LEVEL && self.level < MAX_LEVEL {
            self.xp -= XP_PER_LEVEL;
            self.level += 1;
            gained += 1;
            let new = self.class.stats_at(self.level);
            self.hp += new.max_hp - self.stats.max_hp;
            self.mp += new.max_mp - self.stats.max_mp;
            self.stats = new;
        }
        gained
    }
}

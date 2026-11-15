//! Unità in battaglia e contatori statistici (base per le statistiche persistenti).

use crate::ability::AbilityId;
use crate::class::{Class, MAX_LEVEL, Stats};
use crate::gear::{GearEffects, ItemRef};
use crate::item::sanitize_loadout;
use crate::map::{Dir, Pos};
use crate::status::Status;

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
    /// Equipaggiamento (al massimo un oggetto per slot, adatto alla classe).
    #[cfg_attr(feature = "serde", serde(default))]
    pub equipment: Vec<ItemRef>,
    /// Abilità padroneggiate (di qualsiasi scuola; si usano quelle della classe attuale).
    #[cfg_attr(feature = "serde", serde(default))]
    pub mastered: Vec<AbilityId>,
}

impl UnitSetup {
    pub fn new(name: impl Into<String>, team: u8, class: Class, level: i32) -> Self {
        UnitSetup {
            name: name.into(),
            team,
            class,
            level,
            xp: 0,
            equipment: Vec::new(),
            mastered: Vec::new(),
        }
    }

    pub fn with_equipment(mut self, items: &[impl Into<ItemRef> + Copy]) -> Self {
        self.equipment = items.iter().map(|&i| i.into()).collect();
        self
    }
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
    pub equipment: Vec<ItemRef>,
    pub mastered: Vec<AbilityId>,
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
    /// Status attivi con i turni rimanenti (ordine d'arrivo, deterministico).
    #[cfg_attr(feature = "serde", serde(default))]
    pub statuses: Vec<(Status, i32)>,
}

impl Unit {
    pub fn new(id: UnitId, setup: &UnitSetup, pos: Pos, facing: Dir) -> Self {
        let level = setup.level.clamp(1, MAX_LEVEL);
        let equipment = sanitize_loadout(setup.class, &setup.equipment);
        let stats = stats_with(setup.class, level, &equipment);
        Unit {
            id,
            name: setup.name.clone(),
            team: setup.team,
            class: setup.class,
            level,
            xp: setup.xp.clamp(0, XP_PER_LEVEL - 1),
            equipment,
            mastered: setup.mastered.clone(),
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
            statuses: Vec::new(),
        }
    }

    /// Effetti speciali dell'equipaggiamento (al colpo, resistenze, rigenerazione...).
    pub fn gear_effects(&self) -> GearEffects {
        let mut g = GearEffects::default();
        for it in &self.equipment {
            for e in it.effects() {
                g.add(e);
            }
        }
        g
    }

    /// Abilità utilizzabili ora: Colpo, innate della classe, e quelle della
    /// scuola della classe se padroneggiate o insegnate da un oggetto equipaggiato.
    pub fn abilities(&self) -> Vec<AbilityId> {
        let mut v = vec![AbilityId::Strike];
        v.extend_from_slice(self.class.innate());
        for &a in self.class.learnable() {
            let taught = self.equipment.iter().any(|i| i.teaches() == Some(a));
            if taught || self.mastered.contains(&a) {
                v.push(a);
            }
        }
        v
    }

    pub fn can_use(&self, a: AbilityId) -> bool {
        self.abilities().contains(&a)
    }

    pub fn has(&self, s: Status) -> bool {
        self.statuses.iter().any(|(x, _)| *x == s)
    }

    /// Aggiunge (o rinnova) uno status; annulla quello opposto. Restituisce
    /// lo status opposto rimosso, se c'era.
    pub fn add_status(&mut self, s: Status, turns: i32) -> Option<Status> {
        let removed = s.opposite().filter(|o| self.remove_status(*o));
        match self.statuses.iter_mut().find(|(x, _)| *x == s) {
            Some(e) => e.1 = e.1.max(turns),
            None => self.statuses.push((s, turns)),
        }
        removed
    }

    pub fn remove_status(&mut self, s: Status) -> bool {
        let before = self.statuses.len();
        self.statuses.retain(|(x, _)| *x != s);
        self.statuses.len() != before
    }

    pub fn negative_statuses(&self) -> Vec<Status> {
        self.statuses
            .iter()
            .map(|(s, _)| *s)
            .filter(|s| s.is_negative())
            .collect()
    }

    /// Velocità effettiva per l'iniziativa.
    pub fn effective_speed(&self) -> i32 {
        let mut spd = self.stats.spd.max(1);
        if self.has(Status::Haste) {
            spd = spd * 3 / 2;
        }
        if self.has(Status::Slow) {
            spd /= 2;
        }
        spd.max(1)
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
            let new = stats_with(self.class, self.level, &self.equipment);
            self.hp += new.max_hp - self.stats.max_hp;
            self.mp += new.max_mp - self.stats.max_mp;
            self.stats = new;
        }
        gained
    }
}

/// Statistiche di classe e livello più i bonus dell'equipaggiamento.
pub fn stats_with(class: Class, level: i32, equipment: &[ItemRef]) -> Stats {
    let mut s = equipment
        .iter()
        .fold(class.stats_at(level), |acc, i| acc.plus(i.bonus()));
    s.max_hp = s.max_hp.max(1);
    s.max_mp = s.max_mp.max(0);
    s.spd = s.spd.max(1);
    s.mov = s.mov.max(1);
    s.evade = s.evade.max(0);
    s
}

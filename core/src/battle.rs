//! Stato della battaglia e risoluzione delle azioni.
//!
//! Il modello è "a intenzioni": chi gioca (client umano o IA) propone un'`Action`,
//! `Battle::apply` la valida e la risolve, restituendo gli `Event` prodotti.
//! Tutto è intero e guidato da un solo `Rng`: stesso seed + stesse azioni
//! = stesso stato finale (replay, server autoritativo, anti-cheat).

use std::collections::VecDeque;
use std::fmt;

use crate::ability::{AbilityId, Element, Kind};
use crate::map::{Dir, Map, MapParams, Pos, Terrain};
use crate::rng::Rng;
use crate::statute::{EXPEL_AT, SUSPEND_TURNS, Statute, StatuteCtx};
use crate::unit::{Condition, Unit, UnitId, UnitSetup};

pub const MAX_TURNS: u32 = 400;
pub const CT_READY: i32 = 100;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BattleSetup {
    pub seed: u64,
    /// `None`: parametri della mappa estratti dal seed.
    pub map: Option<MapParams>,
    /// Squadre 0 e 1. L'id di ogni unità è il suo indice in questa lista.
    pub units: Vec<UnitSetup>,
    pub statutes: usize,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    Move(Pos),
    Act {
        ability: AbilityId,
        target: Pos,
    },
    EndTurn(Dir),
    /// La squadra indicata si arrende (valida in qualsiasi momento; il server
    /// accetta solo la resa della squadra di chi la invia).
    Concede(u8),
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    TurnStarted {
        unit: UnitId,
        turn: u32,
    },
    TurnSkipped {
        unit: UnitId,
    },
    Moved {
        unit: UnitId,
        from: Pos,
        to: Pos,
    },
    AbilityUsed {
        unit: UnitId,
        ability: AbilityId,
        target: Pos,
    },
    Damaged {
        unit: UnitId,
        by: UnitId,
        amount: i32,
        hp: i32,
    },
    Healed {
        unit: UnitId,
        by: UnitId,
        amount: i32,
        hp: i32,
    },
    Missed {
        unit: UnitId,
        by: UnitId,
    },
    Defeated {
        unit: UnitId,
    },
    TerrainDamage {
        unit: UnitId,
        terrain: Terrain,
        amount: i32,
        hp: i32,
    },
    StatuteViolated {
        unit: UnitId,
        statute: Statute,
    },
    Suspended {
        unit: UnitId,
        turns: i32,
    },
    Expelled {
        unit: UnitId,
    },
    LevelUp {
        unit: UnitId,
        level: i32,
    },
    BattleEnded {
        winner: Option<u8>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionError {
    BattleOver,
    AlreadyMoved,
    AlreadyActed,
    Unreachable,
    UnknownAbility,
    NotEnoughMp,
    OutOfRange,
}

impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ActionError::BattleOver => "la battaglia è finita",
            ActionError::AlreadyMoved => "l'unità si è già mossa",
            ActionError::AlreadyActed => "l'unità ha già agito",
            ActionError::Unreachable => "casella non raggiungibile",
            ActionError::UnknownAbility => "abilità non disponibile per questa classe",
            ActionError::NotEnoughMp => "MP insufficienti",
            ActionError::OutOfRange => "bersaglio fuori portata",
        };
        f.write_str(s)
    }
}

impl std::error::Error for ActionError {}

/// Effetto previsto su un'unità (senza la variazione casuale).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub target: usize,
    pub hit_pct: i32,
    pub amount: i32,
    pub heal: bool,
}

#[derive(Clone, Debug)]
pub struct Battle {
    pub seed: u64,
    pub map: Map,
    pub units: Vec<Unit>,
    pub statutes: Vec<Statute>,
    rng: Rng,
    turn: u32,
    active: Option<usize>,
    moved: bool,
    acted: bool,
    last_ability: Option<AbilityId>,
    winner: Option<Option<u8>>,
}

/// Stato pubblico della battaglia, senza il generatore casuale: è ciò che il
/// server invia ai client. Ricostruendo una `Battle` da qui si ottengono
/// anteprime e IA, ma non la risoluzione (i tiri restano segreti al server).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub map: Map,
    pub units: Vec<Unit>,
    pub statutes: Vec<Statute>,
    pub turn: u32,
    pub active: Option<usize>,
    pub moved: bool,
    pub acted: bool,
    pub last_ability: Option<AbilityId>,
    pub finished: bool,
    /// Significativo solo se `finished`: `None` = pareggio.
    pub winner: Option<u8>,
}

impl Battle {
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            map: self.map.clone(),
            units: self.units.clone(),
            statutes: self.statutes.clone(),
            turn: self.turn,
            active: self.active,
            moved: self.moved,
            acted: self.acted,
            last_ability: self.last_ability,
            finished: self.winner.is_some(),
            winner: self.winner.flatten(),
        }
    }

    /// Battaglia "di sola consultazione" da uno snapshot: generatore casuale
    /// fittizio, quindi da usare per anteprime e IA lato client, non per giocare.
    pub fn from_snapshot(s: Snapshot) -> Battle {
        Battle {
            seed: 0,
            map: s.map,
            units: s.units,
            statutes: s.statutes,
            rng: Rng::new(0),
            turn: s.turn,
            active: s.active,
            moved: s.moved,
            acted: s.acted,
            last_ability: s.last_ability,
            winner: s.finished.then_some(s.winner),
        }
    }

    pub fn new(setup: &BattleSetup) -> (Self, Vec<Event>) {
        let mut rng = Rng::new(setup.seed);
        let mut map_rng = rng.fork();
        let mut params = setup
            .map
            .clone()
            .unwrap_or_else(|| MapParams::random(&mut map_rng));
        let per_team = (0..2u8)
            .map(|t| setup.units.iter().filter(|u| u.team == t).count())
            .max()
            .unwrap_or(0);
        params.spawns_per_team = params.spawns_per_team.max(per_team);
        let map = Map::generate(&params, &mut map_rng);
        let statutes = Statute::draw(&mut rng.fork(), setup.statutes);

        let mut next = [0usize; 2];
        let mut init_rng = rng.fork();
        let units: Vec<Unit> = setup
            .units
            .iter()
            .enumerate()
            .map(|(i, s)| {
                assert!(s.team < 2, "squadre ammesse: 0 e 1");
                let team = s.team as usize;
                let spawns = &map.spawns[team];
                let pos = spawns[next[team] % spawns.len()];
                next[team] += 1;
                let facing = if team == 0 { Dir::E } else { Dir::W };
                let mut u = Unit::new(i as UnitId, s, pos, facing);
                // Iniziativa casuale: nessuna squadra parte sempre per prima.
                u.ct = init_rng.range(0, 30);
                u
            })
            .collect();

        let mut b = Battle {
            seed: setup.seed,
            map,
            units,
            statutes,
            rng,
            turn: 0,
            active: None,
            moved: false,
            acted: false,
            last_ability: None,
            winner: None,
        };
        let mut ev = Vec::new();
        b.check_victory(&mut ev);
        if b.winner.is_none() {
            b.advance(&mut ev);
        }
        (b, ev)
    }

    pub fn turn(&self) -> u32 {
        self.turn
    }

    pub fn active_index(&self) -> Option<usize> {
        self.active
    }

    pub fn active_unit(&self) -> Option<&Unit> {
        self.active.map(|i| &self.units[i])
    }

    pub fn has_moved(&self) -> bool {
        self.moved
    }

    pub fn has_acted(&self) -> bool {
        self.acted
    }

    pub fn is_over(&self) -> bool {
        self.winner.is_some()
    }

    /// `None` = in corso, `Some(None)` = pareggio, `Some(Some(t))` = vince la squadra `t`.
    pub fn winner(&self) -> Option<Option<u8>> {
        self.winner
    }

    pub fn last_ability(&self) -> Option<AbilityId> {
        self.last_ability
    }

    pub fn unit_at(&self, p: Pos) -> Option<usize> {
        self.units.iter().position(|u| u.is_active() && u.pos == p)
    }

    /// Caselle in cui l'unità può terminare il movimento (inclusa quella attuale).
    pub fn reachable(&self, idx: usize) -> Vec<Pos> {
        let u = &self.units[idx];
        let w = self.map.width;
        let at = |p: Pos| (p.y * w + p.x) as usize;
        let mut best = vec![i32::MAX; (w * self.map.height) as usize];
        best[at(u.pos)] = 0;
        let mut q = VecDeque::from([u.pos]);
        while let Some(p) = q.pop_front() {
            let cost = best[at(p)];
            let h = self.map.tile(p).unwrap().height;
            for n in p.neighbors() {
                let Some(t) = self.map.tile(n) else { continue };
                if (t.height - h).abs() > u.stats.jump {
                    continue;
                }
                if self
                    .unit_at(n)
                    .is_some_and(|o| self.units[o].team != u.team)
                {
                    continue;
                }
                let c = cost + t.terrain.move_cost();
                if c <= u.stats.mov && c < best[at(n)] {
                    best[at(n)] = c;
                    q.push_back(n);
                }
            }
        }
        self.map
            .positions()
            .filter(|&p| best[at(p)] <= u.stats.mov && (p == u.pos || self.unit_at(p).is_none()))
            .collect()
    }

    /// L'unità, trovandosi in `from`, può usare `ability` sulla casella `target`?
    pub fn check_target(
        &self,
        idx: usize,
        from: Pos,
        ability: AbilityId,
        target: Pos,
    ) -> Result<(), ActionError> {
        let u = &self.units[idx];
        if !u.can_use(ability) {
            return Err(ActionError::UnknownAbility);
        }
        let ab = ability.def();
        if u.mp < ab.mp_cost {
            return Err(ActionError::NotEnoughMp);
        }
        let (Some(tt), Some(ft)) = (self.map.tile(target), self.map.tile(from)) else {
            return Err(ActionError::OutOfRange);
        };
        let d = from.dist(target);
        if d < ab.range_min || d > ab.range_max || (tt.height - ft.height).abs() > ab.vertical {
            return Err(ActionError::OutOfRange);
        }
        Ok(())
    }

    /// Statuti che verrebbero violati.
    pub fn violations(&self, from: Pos, ability: AbilityId, target: Pos) -> Vec<Statute> {
        let ctx = StatuteCtx {
            user_height: self.map.tile(from).map_or(0, |t| t.height),
            target_height: self.map.tile(target).map_or(0, |t| t.height),
            previous_ability: self.last_ability,
        };
        let ab = ability.def();
        self.statutes
            .iter()
            .copied()
            .filter(|s| s.is_violated(ab, &ctx))
            .collect()
    }

    /// Effetti previsti (deterministici, senza variazione) dell'abilità.
    pub fn outcomes(&self, idx: usize, from: Pos, ability: AbilityId, target: Pos) -> Vec<Outcome> {
        let ab = ability.def();
        let u = &self.units[idx];
        let from_h = self.map.tile(from).map_or(0, |t| t.height);
        let mut out = Vec::new();
        for (ti, t) in self.units.iter().enumerate() {
            if !t.is_active() {
                continue;
            }
            let tpos = if ti == idx { from } else { t.pos };
            if tpos.dist(target) > ab.aoe {
                continue;
            }
            let tile = self.map.tile(tpos).unwrap();
            let (hit_pct, amount, heal) = match ab.kind {
                Kind::Heal => (100, u.stats.mag * ab.power / 100 * 2, true),
                Kind::Physical => {
                    let raw = u.stats.atk * ab.power / 100 * 2;
                    let def = t.stats.def + tile.terrain.def_bonus();
                    let mut dmg = raw * 100 / (100 + 5 * def);
                    let dh = (from_h - tile.height).clamp(-3, 3);
                    dmg = dmg * (100 + 10 * dh) / 100;
                    let hit = if ti == idx {
                        100
                    } else {
                        (90 + facing_bonus(t.facing, tpos, from)
                            - t.stats.evade
                            - tile.terrain.evade_bonus())
                        .clamp(5, 100)
                    };
                    (hit, dmg, false)
                }
                Kind::Magic => {
                    let raw = u.stats.mag * ab.power / 100 * 2;
                    (100, raw * 100 / (100 + 5 * t.stats.res), false)
                }
            };
            let amount = if heal {
                amount
            } else {
                match (ab.element, tile.terrain) {
                    (Some(Element::Thunder), Terrain::Water) => amount * 3 / 2,
                    (Some(Element::Ice), Terrain::Lava) => amount / 2,
                    (Some(Element::Fire), Terrain::Water) => amount / 2,
                    _ => amount,
                }
            };
            out.push(Outcome {
                target: ti,
                hit_pct,
                amount: amount.max(1),
                heal,
            });
        }
        out
    }

    pub fn apply(&mut self, action: Action) -> Result<Vec<Event>, ActionError> {
        if self.winner.is_some() {
            return Err(ActionError::BattleOver);
        }
        let mut ev = Vec::new();
        if let Action::Concede(team) = action {
            for u in self
                .units
                .iter_mut()
                .filter(|u| u.team == team && u.is_active())
            {
                u.condition = Condition::Expelled;
                ev.push(Event::Expelled { unit: u.id });
            }
            self.check_victory(&mut ev);
            return Ok(ev);
        }
        let idx = self.active.ok_or(ActionError::BattleOver)?;
        match action {
            Action::Concede(_) => unreachable!(),
            Action::Move(dest) => {
                if self.moved {
                    return Err(ActionError::AlreadyMoved);
                }
                if !self.reachable(idx).contains(&dest) {
                    return Err(ActionError::Unreachable);
                }
                let u = &mut self.units[idx];
                let from = u.pos;
                u.pos = dest;
                if dest != from {
                    u.facing = Dir::toward(from, dest);
                }
                self.moved = true;
                ev.push(Event::Moved {
                    unit: u.id,
                    from,
                    to: dest,
                });
            }
            Action::Act { ability, target } => {
                if self.acted {
                    return Err(ActionError::AlreadyActed);
                }
                self.act(idx, ability, target, &mut ev)?;
                self.check_victory(&mut ev);
                if self.winner.is_none() && !self.units[idx].is_active() {
                    self.finish_turn(idx, &mut ev);
                }
            }
            Action::EndTurn(dir) => {
                self.units[idx].facing = dir;
                self.finish_turn(idx, &mut ev);
            }
        }
        Ok(ev)
    }

    fn act(
        &mut self,
        idx: usize,
        ability: AbilityId,
        target: Pos,
        ev: &mut Vec<Event>,
    ) -> Result<(), ActionError> {
        let from = self.units[idx].pos;
        self.check_target(idx, from, ability, target)?;
        let ab = ability.def();
        let violated = self.violations(from, ability, target);
        let outcomes = self.outcomes(idx, from, ability, target);

        self.acted = true;
        self.last_ability = Some(ability);
        let (uid, uteam, ulevel) = {
            let u = &mut self.units[idx];
            u.mp -= ab.mp_cost;
            if target != u.pos {
                u.facing = Dir::toward(u.pos, target);
            }
            u.tally.actions += 1;
            (u.id, u.team, u.level)
        };
        ev.push(Event::AbilityUsed {
            unit: uid,
            ability,
            target,
        });

        let mut xp = 0;
        for o in outcomes {
            if !self.units[o.target].is_active() {
                continue;
            }
            if o.heal {
                let amount = self.vary(o.amount);
                let t = &mut self.units[o.target];
                let amount = amount.min(t.stats.max_hp - t.hp);
                t.hp += amount;
                let (tid, thp) = (t.id, t.hp);
                self.units[idx].tally.healing_done += amount;
                ev.push(Event::Healed {
                    unit: tid,
                    by: uid,
                    amount,
                    hp: thp,
                });
                if amount > 0 {
                    xp += 8;
                }
                continue;
            }
            if !self.rng.percent(o.hit_pct) {
                self.units[idx].tally.misses += 1;
                ev.push(Event::Missed {
                    unit: self.units[o.target].id,
                    by: uid,
                });
                continue;
            }
            let amount = self.vary(o.amount);
            let t = &mut self.units[o.target];
            let amount = amount.min(t.hp);
            t.hp -= amount;
            t.tally.damage_taken += amount;
            let (tid, thp, tteam, tlevel) = (t.id, t.hp, t.team, t.level);
            ev.push(Event::Damaged {
                unit: tid,
                by: uid,
                amount,
                hp: thp,
            });
            let enemy = tteam != uteam;
            if enemy {
                self.units[idx].tally.damage_dealt += amount;
                xp += (10 + 2 * (tlevel - ulevel)).clamp(1, 40);
            }
            if thp == 0 {
                self.units[o.target].condition = Condition::Defeated;
                ev.push(Event::Defeated { unit: tid });
                if enemy {
                    self.units[idx].tally.kills += 1;
                    xp += 20;
                }
            }
            if ab.drain && o.target != idx && self.units[idx].is_active() {
                let u = &mut self.units[idx];
                let gain = (amount / 2).min(u.stats.max_hp - u.hp);
                if gain > 0 {
                    u.hp += gain;
                    ev.push(Event::Healed {
                        unit: uid,
                        by: uid,
                        amount: gain,
                        hp: u.hp,
                    });
                }
            }
        }

        for s in violated {
            self.penalize(idx, s, ev);
        }

        let u = &mut self.units[idx];
        if u.is_active() && xp > 0 && u.gain_xp(xp) > 0 {
            ev.push(Event::LevelUp {
                unit: u.id,
                level: u.level,
            });
        }
        Ok(())
    }

    fn penalize(&mut self, idx: usize, statute: Statute, ev: &mut Vec<Event>) {
        let u = &mut self.units[idx];
        u.penalties += 1;
        u.tally.violations += 1;
        ev.push(Event::StatuteViolated {
            unit: u.id,
            statute,
        });
        if !u.is_active() {
            return;
        }
        if u.penalties >= EXPEL_AT {
            u.condition = Condition::Expelled;
            ev.push(Event::Expelled { unit: u.id });
        } else {
            u.suspended += SUSPEND_TURNS;
            ev.push(Event::Suspended {
                unit: u.id,
                turns: SUSPEND_TURNS,
            });
        }
    }

    fn vary(&mut self, amount: i32) -> i32 {
        (amount * self.rng.range(90, 110) / 100).max(1)
    }

    fn finish_turn(&mut self, idx: usize, ev: &mut Vec<Event>) {
        let terrain = self.map.tile(self.units[idx].pos).unwrap().terrain;
        let u = &mut self.units[idx];
        if u.is_active() {
            let pct = terrain.end_turn_damage_pct();
            if pct > 0 {
                let amount = (u.stats.max_hp * pct / 100).max(1).min(u.hp);
                u.hp -= amount;
                u.tally.damage_taken += amount;
                ev.push(Event::TerrainDamage {
                    unit: u.id,
                    terrain,
                    amount,
                    hp: u.hp,
                });
                if u.hp == 0 {
                    u.condition = Condition::Defeated;
                    ev.push(Event::Defeated { unit: u.id });
                }
            }
        }
        u.ct -= 60 + 20 * self.moved as i32 + 20 * self.acted as i32;
        self.active = None;
        self.check_victory(ev);
        if self.winner.is_none() {
            self.advance(ev);
        }
    }

    /// Fa scorrere il tempo (CT) fino al prossimo turno giocabile.
    fn advance(&mut self, ev: &mut Vec<Event>) {
        loop {
            if self.turn >= MAX_TURNS {
                self.winner = Some(None);
                ev.push(Event::BattleEnded { winner: None });
                return;
            }
            while !self.units.iter().any(|u| u.is_active() && u.ct >= CT_READY) {
                for u in self.units.iter_mut().filter(|u| u.is_active()) {
                    u.ct += u.stats.spd.max(1);
                }
            }
            let mut pick: Option<usize> = None;
            for (i, u) in self.units.iter().enumerate() {
                if u.is_active() && u.ct >= CT_READY && pick.is_none_or(|p| u.ct > self.units[p].ct)
                {
                    pick = Some(i);
                }
            }
            let idx = pick.expect("almeno un'unità pronta");
            self.turn += 1;
            let u = &mut self.units[idx];
            if u.suspended > 0 {
                u.suspended -= 1;
                u.ct -= 60;
                ev.push(Event::TurnSkipped { unit: u.id });
                continue;
            }
            u.mp = (u.mp + (u.stats.max_mp / 10).max(2)).min(u.stats.max_mp);
            self.active = Some(idx);
            self.moved = false;
            self.acted = false;
            ev.push(Event::TurnStarted {
                unit: u.id,
                turn: self.turn,
            });
            return;
        }
    }

    fn check_victory(&mut self, ev: &mut Vec<Event>) {
        if self.winner.is_some() {
            return;
        }
        let alive = |t: u8| self.units.iter().any(|u| u.team == t && u.is_active());
        let w = match (alive(0), alive(1)) {
            (true, true) => return,
            (true, false) => Some(0),
            (false, true) => Some(1),
            (false, false) => None,
        };
        self.winner = Some(w);
        self.active = None;
        ev.push(Event::BattleEnded { winner: w });
    }

    /// Impronta dello stato: due simulazioni concordano se e solo se (in pratica) coincide.
    pub fn state_hash(&self) -> u64 {
        let mut h = Fnv::default();
        h.u64(self.rng.state());
        h.i32(self.turn as i32);
        h.i32(self.active.map_or(-1, |i| i as i32));
        h.i32(self.moved as i32 * 2 + self.acted as i32);
        h.i32(match self.winner {
            None => -2,
            Some(None) => -1,
            Some(Some(t)) => t as i32,
        });
        for u in &self.units {
            for v in [
                u.id as i32,
                u.hp,
                u.mp,
                u.pos.x,
                u.pos.y,
                u.facing as i32,
                u.ct,
                u.level,
                u.xp,
                u.condition as i32,
                u.suspended,
                u.penalties,
            ] {
                h.i32(v);
            }
        }
        h.0
    }
}

/// Bonus al colpire in base al lato da cui arriva l'attacco.
fn facing_bonus(target_facing: Dir, target: Pos, attacker: Pos) -> i32 {
    if target == attacker {
        return 0;
    }
    let toward = Dir::toward(target, attacker);
    let (fx, fy) = target_facing.vec();
    let (ax, ay) = toward.vec();
    match fx * ax + fy * ay {
        1 => 0,   // di fronte
        -1 => 30, // alle spalle
        _ => 15,  // di fianco
    }
}

struct Fnv(u64);

impl Default for Fnv {
    fn default() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
}

impl Fnv {
    fn u64(&mut self, v: u64) {
        for b in v.to_le_bytes() {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }

    fn i32(&mut self, v: i32) {
        self.u64(v as u32 as u64);
    }
}

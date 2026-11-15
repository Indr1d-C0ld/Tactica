//! Stato della battaglia e risoluzione delle azioni.
//!
//! Il modello è "a intenzioni": chi gioca (client umano o IA) propone un'`Action`,
//! `Battle::apply` la valida e la risolve, restituendo gli `Event` prodotti.
//! Tutto è intero e guidato da un solo `Rng`: stesso seed + stesse azioni
//! = stesso stato finale (replay, server autoritativo, anti-cheat).

use std::collections::VecDeque;
use std::fmt;

use crate::ability::{AbilityId, Element, Kind};
use crate::gear::{self, ItemRef, Rarity};
use crate::map::{Dir, Map, MapParams, Pos, Terrain};
use crate::rng::Rng;
use crate::status::{Status, StatusEffect};
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
    /// Condizione di vittoria (predefinita: sconfiggere tutti i nemici).
    #[cfg_attr(feature = "serde", serde(default))]
    pub objective: Objective,
    /// Bottino lasciato dalle unità sconfitte (predefinito: nessuno).
    #[cfg_attr(feature = "serde", serde(default))]
    pub loot: LootRules,
}

/// Regole del bottino, per squadra dell'unità sconfitta.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LootRules {
    /// Probabilità (%) che un'unità sconfitta lasci un oggetto.
    pub chance: [i32; 2],
    /// Spinta verso le rarità alte (0-10).
    pub boost: [i32; 2],
    /// Unità che lascia sempre un oggetto, con una spinta in più (il campione).
    pub champion: Option<UnitId>,
}

/// Oggetto a terra, lasciato da un'unità sconfitta: lo raccoglie chi si ferma
/// sulla casella, purché sia della squadra avversaria.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LootDrop {
    pub pos: Pos,
    pub item: ItemRef,
    /// Squadra dell'unità che l'ha lasciato.
    pub team: u8,
    pub from: UnitId,
}

/// Obiettivo della battaglia.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Objective {
    /// Vince chi resta con unità in campo.
    #[default]
    Rout,
    /// La squadra 0 vince anche abbattendo questa unità (il campione nemico).
    Champion(UnitId),
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
    /// Un'unità sconfitta lascia un oggetto sulla sua casella.
    LootDropped {
        unit: UnitId,
        pos: Pos,
        rarity: Rarity,
    },
    /// Un'unità raccoglie un oggetto fermandosi sulla casella.
    LootPicked {
        unit: UnitId,
        item: ItemRef,
        /// Unità sconfitta che l'aveva lasciato.
        from: UnitId,
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
    StatusApplied {
        unit: UnitId,
        status: Status,
    },
    StatusResisted {
        unit: UnitId,
        status: Status,
    },
    StatusCleared {
        unit: UnitId,
        status: Status,
    },
    StatusExpired {
        unit: UnitId,
        status: Status,
    },
    StatusDamage {
        unit: UnitId,
        status: Status,
        amount: i32,
        hp: i32,
    },
    Asleep {
        unit: UnitId,
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
    Silenced,
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
            ActionError::Silenced => "l'unità è ridotta al silenzio",
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
    /// Status che l'abilità può applicare a questo bersaglio.
    pub status: Option<StatusEffect>,
    /// Rimuove gli status negativi del bersaglio.
    pub cleanse: bool,
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
    pub objective: Objective,
    loot_rules: LootRules,
    /// Oggetti a terra.
    pub loot: Vec<LootDrop>,
    /// Oggetti raccolti, con chi li ha raccolti.
    pub picked: Vec<(UnitId, ItemRef)>,
    /// Unità sconfitte per cui il bottino è già stato estratto.
    loot_rolled: Vec<UnitId>,
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
    #[cfg_attr(feature = "serde", serde(default))]
    pub objective: Objective,
    #[cfg_attr(feature = "serde", serde(default))]
    pub loot: Vec<LootDrop>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub picked: Vec<(UnitId, ItemRef)>,
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
            objective: self.objective,
            loot: self.loot.clone(),
            picked: self.picked.clone(),
        }
    }

    /// Battaglia "di sola consultazione" da uno snapshot: generatore casuale
    /// fittizio, quindi da usare per anteprime e IA lato client, non per giocare.
    pub fn from_snapshot(s: Snapshot) -> Battle {
        let loot_rolled = s
            .units
            .iter()
            .filter(|u| u.condition == Condition::Defeated)
            .map(|u| u.id)
            .collect();
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
            objective: s.objective,
            loot_rules: LootRules::default(),
            loot_rolled,
            loot: s.loot,
            picked: s.picked,
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
            objective: setup.objective,
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
            loot_rules: setup.loot,
            loot: Vec::new(),
            picked: Vec::new(),
            loot_rolled: Vec::new(),
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
        if u.has(Status::Root) {
            return vec![u.pos];
        }
        let (best, _) = self.move_costs(idx);
        let at = |p: Pos| (p.y * self.map.width + p.x) as usize;
        self.map
            .positions()
            .filter(|&p| best[at(p)] <= u.stats.mov && (p == u.pos || self.unit_at(p).is_none()))
            .collect()
    }

    /// Percorso (caselle successive, esclusa la partenza) verso `dest`, se raggiungibile.
    pub fn path(&self, idx: usize, dest: Pos) -> Option<Vec<Pos>> {
        if !self.reachable(idx).contains(&dest) {
            return None;
        }
        let (_, parent) = self.move_costs(idx);
        let at = |p: Pos| (p.y * self.map.width + p.x) as usize;
        let start = self.units[idx].pos;
        let mut out = Vec::new();
        let mut p = dest;
        while p != start {
            out.push(p);
            p = parent[at(p)]?;
        }
        out.reverse();
        Some(out)
    }

    /// Costi minimi di movimento (Dijkstra su griglia) e predecessori.
    fn move_costs(&self, idx: usize) -> (Vec<i32>, Vec<Option<Pos>>) {
        let u = &self.units[idx];
        let w = self.map.width;
        let at = |p: Pos| (p.y * w + p.x) as usize;
        let n = (w * self.map.height) as usize;
        let mut best = vec![i32::MAX; n];
        let mut parent = vec![None; n];
        best[at(u.pos)] = 0;
        let mut q = VecDeque::from([u.pos]);
        while let Some(p) = q.pop_front() {
            let cost = best[at(p)];
            let h = self.map.tile(p).unwrap().height;
            for nb in p.neighbors() {
                let Some(t) = self.map.tile(nb) else { continue };
                if (t.height - h).abs() > u.stats.jump {
                    continue;
                }
                // Ogni unità in piedi occupa la sua casella: non si attraversano
                // né i nemici né gli alleati.
                if self.unit_at(nb).is_some() {
                    continue;
                }
                let c = cost + t.terrain.move_cost();
                if c <= u.stats.mov && c < best[at(nb)] {
                    best[at(nb)] = c;
                    parent[at(nb)] = Some(p);
                    q.push_back(nb);
                }
            }
        }
        (best, parent)
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
        if u.has(Status::Silence) && matches!(ab.kind, Kind::Magic | Kind::Heal | Kind::Support) {
            return Err(ActionError::Silenced);
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
        let gear = u.gear_effects();
        // Gli attacchi fisici senza elemento prendono quello dell'arma.
        let element = ab.element.or(if ab.kind == Kind::Physical {
            gear.attack_element
        } else {
            None
        });
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
            let ally = t.team == u.team;
            // Gli status negativi colpiscono i nemici, quelli positivi gli alleati.
            let status = ab
                .effect
                .filter(|e| e.status.is_negative() != ally)
                .or_else(|| {
                    // Effetti al colpo dell'arma: solo colpi fisici sui nemici.
                    (ab.kind == Kind::Physical && !ally && ti != idx)
                        .then(|| gear.on_hit.first().copied())
                        .flatten()
                });
            let cleanse = ab.cleanse && ally;
            let (hit_pct, amount, heal) = match ab.kind {
                Kind::Support => {
                    if status.is_none() && !cleanse {
                        continue;
                    }
                    (100, 0, false)
                }
                Kind::Heal => (100, u.stats.mag * ab.power / 100 * 2, true),
                Kind::Physical => {
                    let raw = u.stats.atk * ab.power / 100 * 2;
                    let def = t.stats.def + tile.terrain.def_bonus();
                    let mut dmg = raw * 100 / (100 + 5 * def);
                    let dh = (from_h - tile.height).clamp(-3, 3);
                    dmg = dmg * (100 + 10 * dh) / 100;
                    if t.has(Status::Aegis) {
                        dmg = dmg * 2 / 3;
                    }
                    let hit = if ti == idx || t.has(Status::Sleep) {
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
            let amount = if heal || ab.kind == Kind::Support {
                amount
            } else {
                let amount = match (element, tile.terrain) {
                    (Some(Element::Thunder), Terrain::Water) => amount * 3 / 2,
                    (Some(Element::Ice), Terrain::Lava) => amount / 2,
                    (Some(Element::Fire), Terrain::Water) => amount / 2,
                    _ => amount,
                };
                match element {
                    Some(el) if t.gear_effects().resist.contains(&el) => amount / 2,
                    _ => amount,
                }
            };
            out.push(Outcome {
                target: ti,
                hit_pct,
                amount: if ab.kind == Kind::Support {
                    0
                } else {
                    amount.max(1)
                },
                heal,
                status,
                cleanse,
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
                let (uid, team) = (u.id, u.team);
                ev.push(Event::Moved {
                    unit: uid,
                    from,
                    to: dest,
                });
                self.pick_up(uid, team, dest, &mut ev);
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
        self.roll_drops(&mut ev);
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
            if ab.kind == Kind::Support {
                xp += self.apply_support(o, ev);
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
            if thp == 0 {
                self.units[o.target].statuses.clear();
            } else {
                // Il dolore sveglia; poi l'eventuale status dell'abilità.
                if self.units[o.target].remove_status(Status::Sleep) {
                    ev.push(Event::StatusCleared {
                        unit: tid,
                        status: Status::Sleep,
                    });
                }
                if let Some(e) = o.status {
                    self.try_status(o.target, e, ev);
                }
            }
            let gear_drain = if ab.kind == Kind::Physical && enemy {
                self.units[idx].gear_effects().drain_pct
            } else {
                0
            };
            let drain_pct = if ab.drain { 50 } else { 0 } + gear_drain;
            if drain_pct > 0 && o.target != idx && self.units[idx].is_active() {
                let u = &mut self.units[idx];
                let gain = (amount * drain_pct / 100).min(u.stats.max_hp - u.hp);
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

    /// Tira la probabilità e applica uno status; restituisce se è riuscito.
    fn try_status(&mut self, target: usize, e: StatusEffect, ev: &mut Vec<Event>) -> bool {
        let id = self.units[target].id;
        let immune = self.units[target].gear_effects().immune.contains(&e.status);
        if immune || !self.rng.percent(e.chance) {
            ev.push(Event::StatusResisted {
                unit: id,
                status: e.status,
            });
            return false;
        }
        if let Some(gone) = self.units[target].add_status(e.status, e.duration) {
            ev.push(Event::StatusCleared {
                unit: id,
                status: gone,
            });
        }
        ev.push(Event::StatusApplied {
            unit: id,
            status: e.status,
        });
        true
    }

    /// Abilità di supporto: purificazione e/o status. Restituisce l'esperienza.
    fn apply_support(&mut self, o: Outcome, ev: &mut Vec<Event>) -> i32 {
        let mut xp = 0;
        let id = self.units[o.target].id;
        if o.cleanse {
            for s in self.units[o.target].negative_statuses() {
                self.units[o.target].remove_status(s);
                ev.push(Event::StatusCleared {
                    unit: id,
                    status: s,
                });
                xp += 6;
            }
        }
        if let Some(e) = o.status
            && self.try_status(o.target, e, ev)
        {
            xp += 6;
        }
        xp
    }

    /// Effetti di fine turno dell'unità: veleno, rigenerazione, durate.
    fn end_of_turn_effects(&mut self, idx: usize, ev: &mut Vec<Event>) {
        let u = &mut self.units[idx];
        if !u.is_active() {
            return;
        }
        let id = u.id;
        let tick = (u.stats.max_hp * crate::status::TICK_PCT / 100).max(1);
        if u.has(Status::Poison) {
            let amount = tick.min(u.hp);
            u.hp -= amount;
            u.tally.damage_taken += amount;
            ev.push(Event::StatusDamage {
                unit: id,
                status: Status::Poison,
                amount,
                hp: u.hp,
            });
            if u.hp == 0 {
                u.condition = Condition::Defeated;
                u.statuses.clear();
                ev.push(Event::Defeated { unit: id });
                return;
            }
        }
        let gear_regen = u.gear_effects().regen_pct;
        if gear_regen > 0 && !u.has(Status::Regen) {
            let amount = (u.stats.max_hp * gear_regen / 100)
                .max(1)
                .min(u.stats.max_hp - u.hp);
            if amount > 0 {
                u.hp += amount;
                ev.push(Event::Healed {
                    unit: id,
                    by: id,
                    amount,
                    hp: u.hp,
                });
            }
        }
        if u.has(Status::Regen) {
            let amount = tick.min(u.stats.max_hp - u.hp);
            if amount > 0 {
                u.hp += amount;
                ev.push(Event::Healed {
                    unit: id,
                    by: id,
                    amount,
                    hp: u.hp,
                });
            }
        }
        let mut expired = Vec::new();
        for (s, turns) in u.statuses.iter_mut() {
            *turns -= 1;
            if *turns <= 0 {
                expired.push(*s);
            }
        }
        for s in expired {
            u.remove_status(s);
            ev.push(Event::StatusExpired {
                unit: id,
                status: s,
            });
        }
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

    /// Estrae il bottino delle unità appena sconfitte (una volta sola per unità).
    fn roll_drops(&mut self, ev: &mut Vec<Event>) {
        for i in 0..self.units.len() {
            let u = &self.units[i];
            if u.condition != Condition::Defeated || self.loot_rolled.contains(&u.id) {
                continue;
            }
            let (id, team, pos, level) = (u.id, u.team as usize, u.pos, u.level);
            self.loot_rolled.push(id);
            let champion = self.loot_rules.champion == Some(id);
            let chance = if champion {
                100
            } else {
                self.loot_rules.chance[team]
            };
            if chance <= 0 || !self.rng.percent(chance) {
                continue;
            }
            let boost = self.loot_rules.boost[team] + if champion { 4 } else { 0 };
            let item = gear::roll_item(&mut self.rng, level, boost);
            self.loot.push(LootDrop {
                pos,
                item,
                team: team as u8,
                from: id,
            });
            ev.push(Event::LootDropped {
                unit: id,
                pos,
                rarity: item.rarity(),
            });
        }
    }

    /// Raccoglie gli oggetti avversari sulla casella in cui l'unità si è fermata.
    fn pick_up(&mut self, unit: UnitId, team: u8, at: Pos, ev: &mut Vec<Event>) {
        let mut i = 0;
        while i < self.loot.len() {
            if self.loot[i].pos == at && self.loot[i].team != team {
                let d = self.loot.remove(i);
                self.picked.push((unit, d.item));
                ev.push(Event::LootPicked {
                    unit,
                    item: d.item,
                    from: d.from,
                });
            } else {
                i += 1;
            }
        }
    }

    /// Oggetti a terra su una casella.
    pub fn loot_at(&self, p: Pos) -> impl Iterator<Item = &LootDrop> {
        self.loot.iter().filter(move |d| d.pos == p)
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
        self.end_of_turn_effects(idx, ev);
        let u = &mut self.units[idx];
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
                    u.ct += u.effective_speed();
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
            if u.has(Status::Sleep) {
                u.ct -= 60;
                let id = u.id;
                ev.push(Event::Asleep { unit: id });
                self.end_of_turn_effects(idx, ev);
                self.check_victory(ev);
                if self.winner.is_some() {
                    return;
                }
                continue;
            }
            let u = &mut self.units[idx];
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
        let champion_down = match self.objective {
            Objective::Champion(id) => self.units.get(id as usize).is_some_and(|u| !u.is_active()),
            Objective::Rout => false,
        };
        let w = match (alive(0), alive(1)) {
            (true, true) if champion_down => Some(0),
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
            for (st, turns) in &u.statuses {
                h.i32(*st as i32 * 1000 + turns);
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

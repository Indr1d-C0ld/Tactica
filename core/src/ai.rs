//! IA semplice e golosa: valuta ogni (casella, abilità, bersaglio) e sceglie il
//! punteggio atteso migliore, rispettando gli statuti. Serve per PNG in co-op,
//! per riempire squadre e per i test.

use crate::battle::{Action, Battle, Event};
use crate::class::Class;
use crate::map::{Dir, Pos, Terrain};

const VIOLATION_SCORE: i32 = -1000;

pub fn plan_turn(b: &Battle) -> Vec<Action> {
    let Some(idx) = b.active_index() else {
        return Vec::new();
    };
    let u = &b.units[idx];
    let froms = if b.has_moved() {
        vec![u.pos]
    } else {
        b.reachable(idx)
    };

    let mut best: Option<(i32, Pos, Action)> = None;
    if !b.has_acted() {
        for &from in &froms {
            let lava = if is_lava(b, from) { 15 } else { 0 };
            for ability in u.abilities() {
                let r = ability.def().range_max;
                for dy in -r..=r {
                    for dx in -r..=r {
                        let target = Pos::new(from.x + dx, from.y + dy);
                        if b.check_target(idx, from, ability, target).is_err() {
                            continue;
                        }
                        let s = score(b, idx, from, ability, target) - lava;
                        if s > 0 && best.as_ref().is_none_or(|(bs, ..)| s > *bs) {
                            best = Some((s, from, Action::Act { ability, target }));
                        }
                    }
                }
            }
        }
    }

    let mut plan = Vec::new();
    let end_pos = match best {
        Some((_, from, act)) => {
            if from != u.pos {
                plan.push(Action::Move(from));
            }
            plan.push(act);
            from
        }
        None if !b.has_moved() => {
            let dest = position_for(b, idx, &froms);
            if dest != u.pos {
                plan.push(Action::Move(dest));
            }
            dest
        }
        None => u.pos,
    };
    let facing = nearest_enemy(b, idx, end_pos).map_or(u.facing, |e| Dir::toward(end_pos, e));
    plan.push(Action::EndTurn(facing));
    plan
}

/// Gioca automaticamente finché la battaglia non finisce o si superano `max_actions`.
/// Restituisce le azioni (il "replay") e gli eventi.
pub fn autoplay(b: &mut Battle, max_actions: usize) -> (Vec<Action>, Vec<Event>) {
    let mut actions = Vec::new();
    let mut events = Vec::new();
    while !b.is_over() && actions.len() < max_actions {
        let turn = b.turn();
        let plan = plan_turn(b);
        if plan.is_empty() {
            break;
        }
        for a in plan {
            if b.is_over() || b.turn() != turn {
                break;
            }
            let ev = b.apply(a).expect("l'IA ha proposto un'azione non valida");
            actions.push(a);
            events.extend(ev);
        }
    }
    (actions, events)
}

fn is_lava(b: &Battle, p: Pos) -> bool {
    b.map.tile(p).is_some_and(|t| t.terrain == Terrain::Lava)
}

fn score(
    b: &Battle,
    idx: usize,
    from: Pos,
    ability: crate::ability::AbilityId,
    target: Pos,
) -> i32 {
    if !b.violations(from, ability, target).is_empty() {
        return VIOLATION_SCORE;
    }
    let u = &b.units[idx];
    let mut s = 0;
    for o in b.outcomes(idx, from, ability, target) {
        let t = &b.units[o.target];
        let ally = t.team == u.team;
        if o.heal {
            let eff = o.amount.min(t.stats.max_hp - t.hp);
            s += if ally { eff * 3 / 2 } else { -eff };
        } else {
            let expected = o.amount * o.hit_pct / 100;
            let lethal = o.amount >= t.hp;
            if ally {
                s -= expected * 2 + if lethal { 100 } else { 0 };
            } else {
                s += expected + if lethal { 40 * o.hit_pct / 100 } else { 0 };
                if ability.def().drain {
                    s += (expected / 2).min(u.stats.max_hp - u.hp);
                }
            }
        }
    }
    s - ability.def().mp_cost / 2
}

fn nearest_enemy(b: &Battle, idx: usize, from: Pos) -> Option<Pos> {
    let team = b.units[idx].team;
    b.units
        .iter()
        .filter(|e| e.is_active() && e.team != team)
        .min_by_key(|e| (from.dist(e.pos), e.id))
        .map(|e| e.pos)
}

/// Posizionamento quando non c'è nulla di utile da fare.
fn position_for(b: &Battle, idx: usize, froms: &[Pos]) -> Pos {
    let u = &b.units[idx];
    let wounded_ally = b
        .units
        .iter()
        .filter(|a| a.is_active() && a.team == u.team && a.id != u.id && a.hp < a.stats.max_hp)
        .min_by_key(|a| (a.hp * 100 / a.stats.max_hp, a.id))
        .map(|a| a.pos);
    let (anchor, preferred) = match u.class {
        Class::Mender => match wounded_ally {
            Some(p) => (Some(p), 2),
            None => (nearest_enemy(b, idx, u.pos), 4),
        },
        Class::Vanguard | Class::Runeblade | Class::Warden => (nearest_enemy(b, idx, u.pos), 1),
        Class::Scout | Class::Arcanist => (nearest_enemy(b, idx, u.pos), 3),
    };
    let Some(anchor) = anchor else { return u.pos };
    froms
        .iter()
        .copied()
        .min_by_key(|&p| {
            let lava = if is_lava(b, p) { 5 } else { 0 };
            (p.dist(anchor) - preferred).abs() * 2 + lava
        })
        .unwrap_or(u.pos)
}

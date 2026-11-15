use tactica_core::ability::AbilityId;
use tactica_core::ai;
use tactica_core::class::Class;
use tactica_core::map::{Biome, Map, MapParams};
use tactica_core::presets;
use tactica_core::rng::Rng;
use tactica_core::statute::Statute;
use tactica_core::{Action, ActionError, Battle, BattleSetup, Event, Pos, UnitSetup};

fn setup(seed: u64) -> BattleSetup {
    let mut units = presets::demo_party(0, 3, ["A", "B", "C", "D"]);
    units.extend(presets::demo_party(1, 3, ["a", "b", "c", "d"]));
    BattleSetup {
        seed,
        map: None,
        units,
        statutes: 2,
        objective: Default::default(),
        loot: Default::default(),
    }
}

#[test]
fn same_seed_same_battle() {
    for seed in [1, 42, 9_999, u64::MAX] {
        let (mut a, _) = Battle::new(&setup(seed));
        let (mut b, _) = Battle::new(&setup(seed));
        let (acts_a, ev_a) = ai::autoplay(&mut a, 10_000);
        let (acts_b, ev_b) = ai::autoplay(&mut b, 10_000);
        assert_eq!(acts_a, acts_b);
        assert_eq!(ev_a, ev_b);
        assert_eq!(a.state_hash(), b.state_hash());
    }
}

#[test]
fn replay_reproduces_state() {
    for seed in 0..40 {
        let s = setup(seed);
        let (mut live, _) = Battle::new(&s);
        let (actions, _) = ai::autoplay(&mut live, 10_000);
        assert!(live.is_over(), "seed {seed}: la battaglia non è terminata");
        let (mut replay, _) = Battle::new(&s);
        for a in actions {
            replay.apply(a).unwrap();
        }
        assert_eq!(live.state_hash(), replay.state_hash(), "seed {seed}");
    }
}

#[test]
fn different_seeds_give_different_maps() {
    let maps: Vec<Map> = (0..20).map(|s| Battle::new(&setup(s)).0.map).collect();
    let distinct = (0..maps.len())
        .filter(|&i| (0..i).all(|j| maps[i] != maps[j]))
        .count();
    assert!(distinct >= 19, "solo {distinct} mappe distinte su 20");
}

#[test]
fn generated_maps_are_connected() {
    let mut rng = Rng::new(7);
    for _ in 0..300 {
        let params = MapParams::random(&mut rng);
        let map = Map::generate(&params, &mut rng.fork());
        assert!(map.spawns_connected());
        assert_eq!(map.spawns[0].len(), params.spawns_per_team);
        for p in map.spawns.iter().flatten() {
            assert!(map.tile(*p).unwrap().standable());
        }
    }
}

#[test]
fn rejects_illegal_actions() {
    let (mut b, _) = Battle::new(&setup(3));
    let idx = b.active_index().unwrap();
    let far = Pos::new(-5, -5);
    assert_eq!(b.apply(Action::Move(far)), Err(ActionError::Unreachable));
    // Abilità di un'altra classe.
    let foreign = match b.units[idx].class {
        Class::Vanguard => AbilityId::Bolt,
        _ => AbilityId::HeavyBlow,
    };
    let pos = b.units[idx].pos;
    assert_eq!(
        b.apply(Action::Act {
            ability: foreign,
            target: pos
        }),
        Err(ActionError::UnknownAbility)
    );
    let ok = b.reachable(idx)[0];
    b.apply(Action::Move(ok)).unwrap();
    assert_eq!(b.apply(Action::Move(ok)), Err(ActionError::AlreadyMoved));
}

#[test]
fn violating_a_statute_suspends_then_expels() {
    // Mappa piatta e un solo statuto noto: "Niente cure".
    let params = MapParams {
        width: 10,
        height: 10,
        biome: Biome::Meadow,
        max_height: 0,
        roughness: 0,
        spawns_per_team: 2,
    };
    let units = vec![
        UnitSetup::new("M", 0, Class::Mender, 1),
        UnitSetup::new("X", 1, Class::Vanguard, 1),
    ];
    // Cerca un seed in cui lo statuto estratto è NoHealing e tocca al Guaritore.
    let (mut b, _) = (0..10_000)
        .map(|seed| {
            Battle::new(&BattleSetup {
                seed,
                map: Some(params.clone()),
                units: units.clone(),
                statutes: 1,
                objective: Default::default(),
                loot: Default::default(),
            })
        })
        .find(|(b, _)| b.statutes == [Statute::NoHealing] && b.active_index() == Some(0))
        .expect("seed adatto");

    let me = b.units[0].pos;
    let ev = b
        .apply(Action::Act {
            ability: AbilityId::Mend,
            target: me,
        })
        .unwrap();
    assert!(ev.contains(&Event::StatuteViolated {
        unit: 0,
        statute: Statute::NoHealing
    }));
    assert!(ev.contains(&Event::Suspended { unit: 0, turns: 2 }));
    assert_eq!(b.units[0].suspended, 2);

    // Chiude il turno e scorre finché il Guaritore torna attivo, poi seconda violazione.
    let ev = b.apply(Action::EndTurn(b.units[0].facing)).unwrap();
    let mut skipped = ev
        .iter()
        .filter(|e| matches!(e, Event::TurnSkipped { unit: 0 }))
        .count();
    while b.active_index() != Some(0) {
        let ev = b
            .apply(Action::EndTurn(b.units[b.active_index().unwrap()].facing))
            .unwrap();
        skipped += ev
            .iter()
            .filter(|e| matches!(e, Event::TurnSkipped { unit: 0 }))
            .count();
    }
    assert_eq!(skipped, 2);
    let me = b.units[0].pos;
    let ev = b
        .apply(Action::Act {
            ability: AbilityId::Mend,
            target: me,
        })
        .unwrap();
    assert!(ev.contains(&Event::Expelled { unit: 0 }));
    assert_eq!(b.winner(), Some(Some(1)));
}

#[test]
fn ai_respects_statutes() {
    let mut violations = 0;
    for seed in 0..40 {
        let (mut b, _) = Battle::new(&setup(seed));
        ai::autoplay(&mut b, 10_000);
        violations += b.units.iter().map(|u| u.tally.violations).sum::<i32>();
    }
    assert_eq!(violations, 0);
}

#[test]
fn snapshot_roundtrip_keeps_previews() {
    let (mut b, _) = Battle::new(&setup(11));
    ai::autoplay(&mut b, 30);
    let view = Battle::from_snapshot(b.snapshot());
    assert_eq!(view.snapshot(), b.snapshot());
    if let Some(idx) = b.active_index() {
        assert_eq!(view.reachable(idx), b.reachable(idx));
        assert_eq!(ai::plan_turn(&view), ai::plan_turn(&b));
    }
}

#[test]
fn concede_ends_battle_and_replays() {
    let s = setup(5);
    let (mut b, _) = Battle::new(&s);
    let (mut actions, _) = ai::autoplay(&mut b, 12);
    let team = b.active_unit().map_or(0, |u| 1 - u.team);
    b.apply(Action::Concede(team)).unwrap();
    actions.push(Action::Concede(team));
    assert_eq!(b.winner(), Some(Some(1 - team)));
    let (mut r, _) = Battle::new(&s);
    for a in actions {
        r.apply(a).unwrap();
    }
    assert_eq!(r.state_hash(), b.state_hash());
}

#[test]
fn equipment_teaches_and_boosts() {
    use tactica_core::item::ItemId;
    use tactica_core::unit::{Unit, stats_with};
    use tactica_core::{Dir, Pos};
    let bare = UnitSetup::new("A", 0, Class::Arcanist, 5);
    let geared = bare
        .clone()
        .with_equipment(&[ItemId::FrostRod, ItemId::Robe]);
    let u0 = Unit::new(0, &bare, Pos::new(0, 0), Dir::E);
    let u1 = Unit::new(0, &geared, Pos::new(0, 0), Dir::E);
    assert!(!u0.can_use(AbilityId::Frost));
    assert!(u1.can_use(AbilityId::Frost));
    assert_eq!(u1.stats.mag, u0.stats.mag + 4);
    assert_eq!(
        u1.stats,
        tactica_core::people::People::Human.apply(stats_with(
            Class::Arcanist,
            5,
            &[ItemId::FrostRod, ItemId::Robe].map(tactica_core::gear::ItemRef::from)
        ))
    );
    // Padroneggiata: resta anche senza l'oggetto, ma solo nella propria classe.
    let mut m = bare.clone();
    m.mastered = vec![AbilityId::Frost];
    assert!(Unit::new(0, &m, Pos::new(0, 0), Dir::E).can_use(AbilityId::Frost));
    m.class = Class::Mender;
    assert!(!Unit::new(0, &m, Pos::new(0, 0), Dir::E).can_use(AbilityId::Frost));
    // Oggetti non adatti alla classe vengono ignorati.
    let wrong = UnitSetup::new("V", 0, Class::Vanguard, 1).with_equipment(&[ItemId::FrostRod]);
    assert!(
        Unit::new(0, &wrong, Pos::new(0, 0), Dir::E)
            .equipment
            .is_empty()
    );
}

#[test]
fn advanced_classes_fight() {
    use tactica_core::item::ItemId;
    let units = vec![
        UnitSetup::new("R", 0, Class::Runeblade, 6)
            .with_equipment(&[ItemId::RuneSaber, ItemId::Chainmail]),
        UnitSetup::new("W", 0, Class::Warden, 6)
            .with_equipment(&[ItemId::VotiveMace, ItemId::WardenPlate]),
        UnitSetup::new("V", 1, Class::Vanguard, 6).with_equipment(&[ItemId::Greatblade]),
        UnitSetup::new("S", 1, Class::Scout, 6).with_equipment(&[ItemId::Hailbow]),
    ];
    let mut used = std::collections::HashSet::new();
    for seed in 0..30 {
        let (mut b, _) = Battle::new(&BattleSetup {
            seed,
            map: None,
            units: units.clone(),
            statutes: 1,
            objective: Default::default(),
            loot: Default::default(),
        });
        let (_, ev) = ai::autoplay(&mut b, 10_000);
        assert!(b.is_over());
        for e in ev {
            if let Event::AbilityUsed { ability, .. } = e {
                used.insert(ability);
            }
        }
    }
    for a in [
        AbilityId::DrainBlade,
        AbilityId::RuneEdge,
        AbilityId::Judgment,
        AbilityId::Volley,
    ] {
        assert!(used.contains(&a), "{a:?} mai usata");
    }
}

#[test]
fn missions_scale_and_champion_wins() {
    use tactica_core::Objective;
    let allies = presets::demo_party(0, 5, ["A", "B", "C", "D"]);
    let m1 = presets::mission(7, &allies, 1);
    let m3 = presets::mission(7, &allies, 3);
    assert_eq!(m1, presets::mission(7, &allies, 1), "deterministica");
    assert!(m3.enemies.len() > m1.enemies.len());
    assert!(m3.enemies.iter().map(|e| e.level).max() > m1.enemies.iter().map(|e| e.level).max());
    assert!(m1.enemies.iter().all(|e| e.team == 1));

    // Abbattere il campione basta per vincere, anche con altri nemici in piedi.
    let mut early_wins = 0;
    for seed in 0..60 {
        let m = presets::mission(seed, &allies, 1);
        let Some(c) = m.champion else { continue };
        let champ_id = (allies.len() + c) as u32;
        let mut units = allies.clone();
        units.extend(m.enemies.clone());
        let setup = BattleSetup {
            seed,
            map: None,
            units,
            statutes: 0,
            objective: Objective::Champion(champ_id),
            loot: Default::default(),
        };
        let (mut b, _) = Battle::new(&setup);
        let (_, ev) = ai::autoplay(&mut b, 20_000);
        let champ_down = ev
            .iter()
            .any(|e| matches!(e, Event::Defeated { unit } if *unit == champ_id));
        if champ_down {
            assert_eq!(b.winner(), Some(Some(0)), "seed {seed}");
            if b.units.iter().any(|u| u.team == 1 && u.is_active()) {
                early_wins += 1;
            }
        }
    }
    assert!(
        early_wins > 0,
        "nessuna vittoria anticipata per caduta del campione"
    );
}

mod statuses {
    use super::*;
    use tactica_core::status::Status;

    fn status_setup(seed: u64) -> BattleSetup {
        let mut units = presets::status_party(0, 4, ["A", "B", "C", "D"]);
        units.extend(presets::demo_party(1, 4, ["a", "b", "c", "d"]));
        BattleSetup {
            seed,
            map: None,
            units,
            statutes: 1,
            objective: Default::default(),
            loot: Default::default(),
        }
    }

    #[test]
    fn root_and_silence() {
        let (mut b, _) = Battle::new(&setup(21));
        let idx = b.active_index().unwrap();
        b.units[idx].add_status(Status::Root, 2);
        assert_eq!(b.reachable(idx), vec![b.units[idx].pos]);
        // Silenzio: niente incantesimi (un Colpo resta possibile).
        let u = &mut b.units[idx];
        u.add_status(Status::Silence, 2);
        u.class = Class::Mender;
        u.equipment.clear();
        let pos = u.pos;
        assert_eq!(
            b.check_target(idx, pos, AbilityId::Mend, pos),
            Err(ActionError::Silenced)
        );
    }

    #[test]
    fn poison_ticks_and_statuses_expire() {
        let (mut b, _) = Battle::new(&setup(22));
        let idx = b.active_index().unwrap();
        let max = b.units[idx].stats.max_hp;
        b.units[idx].add_status(Status::Poison, 1);
        let hp = b.units[idx].hp;
        let facing = b.units[idx].facing;
        let ev = b.apply(Action::EndTurn(facing)).unwrap();
        assert!(ev.iter().any(|e| matches!(
            e,
            Event::StatusDamage {
                status: Status::Poison,
                ..
            }
        )));
        assert_eq!(b.units[idx].hp, hp - (max * 8 / 100).max(1));
        assert!(ev.iter().any(|e| matches!(
            e,
            Event::StatusExpired {
                status: Status::Poison,
                ..
            }
        )));
        assert!(!b.units[idx].has(Status::Poison));
    }

    #[test]
    fn sleepers_skip_turns_and_haste_cancels_slow() {
        let (mut b, _) = Battle::new(&setup(23));
        let first = b.active_index().unwrap();
        // Addormenta tutti gli altri: i loro turni vengono saltati.
        let sleeper = (first + 1) % b.units.len();
        b.units[sleeper].add_status(Status::Sleep, 2);
        let mut skipped = 0;
        for _ in 0..40 {
            let a = b.active_index().unwrap();
            let f = b.units[a].facing;
            let ev = b.apply(Action::EndTurn(f)).unwrap();
            skipped += ev
                .iter()
                .filter(|e| matches!(e, Event::Asleep { unit } if *unit as usize == sleeper))
                .count();
            if !b.units[sleeper].has(Status::Sleep) {
                break;
            }
        }
        assert_eq!(skipped, 2, "dorme per due dei suoi turni");

        let u = &mut b.units[first];
        u.add_status(Status::Haste, 3);
        assert_eq!(u.add_status(Status::Slow, 3), Some(Status::Haste));
        assert!(u.has(Status::Slow) && !u.has(Status::Haste));
    }

    #[test]
    fn status_battles_are_deterministic_and_use_statuses() {
        let mut seen = std::collections::HashSet::new();
        for seed in 0..25 {
            let s = status_setup(seed);
            let (mut live, _) = Battle::new(&s);
            let (actions, events) = ai::autoplay(&mut live, 20_000);
            assert!(live.is_over());
            for e in &events {
                if let Event::StatusApplied { status, .. } = e {
                    seen.insert(*status);
                }
            }
            let (mut replay, _) = Battle::new(&s);
            for a in actions {
                replay.apply(a).unwrap();
            }
            assert_eq!(live.state_hash(), replay.state_hash(), "seed {seed}");
        }
        for s in [Status::Poison, Status::Sleep, Status::Haste, Status::Regen] {
            assert!(seen.contains(&s), "{s:?} mai applicato; visti {seen:?}");
        }
    }
}

#[test]
fn paths_follow_reachable_tiles() {
    let (b, _) = Battle::new(&setup(31));
    let idx = b.active_index().unwrap();
    for dest in b.reachable(idx) {
        let path = b.path(idx, dest).expect("percorso");
        let mut prev = b.units[idx].pos;
        for step in &path {
            assert_eq!(prev.dist(*step), 1, "passi adiacenti");
            prev = *step;
        }
        assert_eq!(prev, dest);
    }
    assert!(b.path(idx, Pos::new(-1, -1)).is_none());
}

#[test]
fn units_block_movement_paths() {
    // Nessun percorso attraversa una casella occupata, alleata o nemica,
    // e nessuna destinazione è occupata da un'altra unità.
    for seed in 0..30 {
        let (mut b, _) = Battle::new(&setup(seed));
        for _ in 0..12 {
            let Some(idx) = b.active_index() else { break };
            for dest in b.reachable(idx) {
                let path = b.path(idx, dest).expect("destinazione raggiungibile");
                for step in &path {
                    assert!(
                        b.unit_at(*step).is_none_or(|o| o == idx),
                        "seed {seed}: il percorso attraversa un'unità in {step:?}"
                    );
                }
            }
            let Some(plan) = ai::plan_turn(&b).into_iter().next() else {
                break;
            };
            if b.apply(plan).is_err() {
                break;
            }
        }
    }
}

fn loot_setup(seed: u64) -> BattleSetup {
    let mut s = setup(seed);
    s.loot = tactica_core::LootRules {
        chance: [100, 100],
        boost: [3, 3],
        champion: None,
    };
    s
}

#[test]
fn defeated_units_drop_loot_and_enemies_pick_it_up() {
    let mut dropped = 0;
    let mut picked = 0;
    for seed in 0..25 {
        let (mut b, _) = Battle::new(&loot_setup(seed));
        let (_, events) = ai::autoplay(&mut b, 10_000);
        let defeated: Vec<_> = b
            .units
            .iter()
            .filter(|u| u.condition == tactica_core::unit::Condition::Defeated)
            .map(|u| u.id)
            .collect();
        for e in &events {
            match e {
                Event::LootDropped { unit, .. } => {
                    dropped += 1;
                    assert!(defeated.contains(unit));
                }
                Event::LootPicked { unit, item, from } => {
                    picked += 1;
                    assert!(b.picked.contains(&(*unit, *item)));
                    // Lo raccoglie solo un avversario di chi l'ha lasciato.
                    assert_ne!(b.units[*unit as usize].team, b.units[*from as usize].team);
                }
                _ => {}
            }
        }
        // Ogni sconfitto (con probabilità 100%) lascia esattamente un oggetto.
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::LootDropped { .. }))
                .count(),
            defeated.len()
        );
    }
    assert!(dropped > 50, "poco bottino: {dropped}");
    assert!(picked > 5, "l'IA non raccoglie quasi mai: {picked}");
}

#[test]
fn loot_is_deterministic_and_survives_snapshots() {
    let (mut a, _) = Battle::new(&loot_setup(3));
    let (mut b, _) = Battle::new(&loot_setup(3));
    ai::autoplay(&mut a, 10_000);
    ai::autoplay(&mut b, 10_000);
    assert_eq!(a.picked, b.picked);
    assert_eq!(a.loot, b.loot);
    let view = Battle::from_snapshot(a.snapshot());
    assert_eq!(view.loot, a.loot);
    assert_eq!(view.snapshot(), a.snapshot());
}

#[test]
fn no_loot_without_rules() {
    let (mut b, _) = Battle::new(&setup(5));
    let (_, events) = ai::autoplay(&mut b, 10_000);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::LootDropped { .. }))
    );
}

mod consumables {
    use super::*;
    use tactica_core::consumable::{CONSUMABLES, Consumable};
    use tactica_core::gear::ItemRef;
    use tactica_core::status::Status;
    use tactica_core::unit::Condition;

    fn item(code: &str, tier: u8) -> ItemRef {
        let kind = CONSUMABLES.iter().position(|c| c.code == code).unwrap() as u8;
        ItemRef::Use(Consumable { kind, tier })
    }

    /// Battaglia con l'unità attiva pronta, senza statuti.
    fn ready(seed: u64) -> (Battle, usize) {
        let mut s = setup(seed);
        s.statutes = 0;
        let (b, _) = Battle::new(&s);
        let idx = b.active_index().unwrap();
        (b, idx)
    }

    #[test]
    fn potion_heals_and_is_consumed() {
        let (mut b, idx) = ready(1);
        b.units[idx].hp = 10;
        b.units[idx].bag = vec![Some(item("pozione", 0))];
        let me = b.units[idx].pos;
        let ev = b
            .apply(Action::UseItem {
                slot: 0,
                target: me,
            })
            .unwrap();
        assert!(ev.iter().any(|e| matches!(e, Event::ItemUsed { .. })));
        assert!(b.units[idx].hp > 10);
        assert_eq!(b.units[idx].bag, vec![None]);
        assert_eq!(b.consumed.len(), 1);
        // Una seconda volta non c'è più nulla, e l'azione è già stata usata.
        assert!(
            b.apply(Action::UseItem {
                slot: 0,
                target: me
            })
            .is_err()
        );
    }

    #[test]
    fn scrolls_work_for_any_class_but_not_under_silence() {
        let (mut b, idx) = ready(2);
        let me = b.units[idx].pos;
        b.units[idx].bag = vec![Some(item("p_cura", 2)), Some(item("pozione", 0))];
        b.units[idx].hp = 5;
        b.units[idx].add_status(Status::Silence, 3);
        assert_eq!(
            b.apply(Action::UseItem {
                slot: 0,
                target: me
            }),
            Err(ActionError::Silenced)
        );
        // La pozione invece si beve anche in silenzio.
        assert!(
            b.apply(Action::UseItem {
                slot: 1,
                target: me
            })
            .is_ok()
        );

        let (mut b, idx) = ready(3);
        let me = b.units[idx].pos;
        b.units[idx].mp = 0;
        b.units[idx].hp = 5;
        b.units[idx].bag = vec![Some(item("p_cura", 2))];
        // Senza MP e senza conoscere Cura: la pergamena funziona lo stesso.
        assert!(
            b.apply(Action::UseItem {
                slot: 0,
                target: me
            })
            .is_ok()
        );
        assert!(b.units[idx].hp > 5);
    }

    #[test]
    fn revive_brings_back_an_adjacent_defeated_ally() {
        let (mut b, idx) = ready(4);
        let me = b.units[idx].pos;
        let team = b.units[idx].team;
        // Un alleato sconfitto su una casella libera accanto.
        let ally = (0..b.units.len())
            .find(|&i| i != idx && b.units[i].team == team)
            .unwrap();
        let spot = me
            .neighbors()
            .into_iter()
            .find(|&p| b.map.tile(p).is_some() && b.unit_at(p).is_none())
            .expect("casella libera accanto");
        b.units[ally].pos = spot;
        b.units[ally].hp = 0;
        b.units[ally].condition = Condition::Defeated;
        b.units[idx].bag = vec![Some(item("essenza", 0))];
        let ev = b
            .apply(Action::UseItem {
                slot: 0,
                target: spot,
            })
            .unwrap();
        assert!(ev.iter().any(|e| matches!(e, Event::Revived { .. })));
        assert!(b.units[ally].is_active() && b.units[ally].hp > 0);
    }

    #[test]
    fn ai_uses_consumables_and_battles_stay_deterministic() {
        let with_bags = |seed| {
            let mut s = setup(seed);
            for u in &mut s.units {
                u.bag = vec![item("pozione", 1), item("bomba", 1)];
            }
            s
        };
        let mut used = 0;
        for seed in 0..15 {
            let (mut a, _) = Battle::new(&with_bags(seed));
            let (mut c, _) = Battle::new(&with_bags(seed));
            ai::autoplay(&mut a, 10_000);
            ai::autoplay(&mut c, 10_000);
            assert_eq!(a.state_hash(), c.state_hash());
            used += a.consumed.len();
        }
        assert!(used > 10, "l'IA non usa quasi mai i consumabili: {used}");
    }
}

mod advanced_classes {
    use super::*;
    use tactica_core::class::Class;
    use tactica_core::consumable::bag_slots;

    #[test]
    fn every_class_has_kit_school_and_requirement() {
        for c in Class::ALL {
            let kit = presets::class_kit(c);
            assert_eq!(kit.len(), 2, "{c:?}: kit incompleto");
            assert!(
                tactica_core::item::validate_loadout(c, &kit).is_ok(),
                "{c:?}"
            );
            assert!(!c.innate().is_empty() && c.learnable().len() >= 3, "{c:?}");
            if !c.is_base() {
                assert!(c.requirement().is_some());
            }
        }
        assert_eq!(bag_slots(Class::Alchemist), 4);
        assert_eq!(bag_slots(Class::Scout), 2);
    }

    #[test]
    fn mixed_parties_with_new_classes_play_to_the_end() {
        let new = [
            Class::Shadowblade,
            Class::Dragoon,
            Class::Bard,
            Class::Alchemist,
            Class::Geomancer,
            Class::Summoner,
        ];
        for seed in 0..12u64 {
            let mut units = Vec::new();
            for team in 0..2u8 {
                for k in 0..4 {
                    let c = new[(seed as usize + k + team as usize * 2) % new.len()];
                    let mut u = UnitSetup::new(format!("u{team}{k}"), team, c, 12)
                        .with_equipment(&presets::class_kit(c));
                    u.mastered = c.learnable().to_vec();
                    units.push(u);
                }
            }
            let (mut b, _) = Battle::new(&BattleSetup {
                seed,
                map: None,
                units,
                statutes: 2,
                objective: Default::default(),
                loot: Default::default(),
            });
            ai::autoplay(&mut b, 20_000);
            assert!(b.is_over(), "seed {seed}: la battaglia non finisce");
        }
    }
}

mod movement {
    use super::*;
    use tactica_core::class::Class;
    use tactica_core::map::Terrain;
    use tactica_core::people::People;
    use tactica_core::status::Status;
    use tactica_core::{Dir, Unit};

    fn unit(class: Class, people: People) -> Unit {
        let mut s = UnitSetup::new("u", 0, class, 5);
        s.people = people;
        Unit::new(0, &s, Pos::new(0, 0), Dir::E)
    }

    #[test]
    fn statuses_change_movement() {
        let mut u = unit(Class::Scout, People::Human);
        let base = u.move_points();
        u.add_status(Status::Haste, 2);
        assert_eq!(u.move_points(), base + 1);
        u.add_status(Status::Slow, 2); // annulla Rapidità
        assert_eq!(u.move_points(), base - 1);
        u.add_status(Status::Burden, 2);
        assert_eq!(u.move_points(), (base - 3).max(1));
        u.add_status(Status::Root, 2);
        assert_eq!(u.move_points(), 0);
        let mut f = unit(Class::Arcanist, People::Human);
        f.add_status(Status::Flight, 2);
        assert!(f.can_step(0, 9));
        assert_eq!(f.terrain_cost(Terrain::Water), 1);
    }

    #[test]
    fn peoples_and_classes_change_terrain_and_reach() {
        let h = unit(Class::Vanguard, People::Human);
        let s = unit(Class::Vanguard, People::Sylvan);
        let r = unit(Class::Vanguard, People::Stonekin);
        let w = unit(Class::Vanguard, People::Winged);
        let o = unit(Class::Vanguard, People::Undine);
        assert_eq!(s.move_points(), h.move_points() + 1);
        assert_eq!(r.move_points(), h.move_points() - 1);
        assert_eq!(w.stats.jump, h.stats.jump + 2);
        assert_eq!(h.terrain_cost(Terrain::Thicket), 2);
        assert_eq!(s.terrain_cost(Terrain::Thicket), 1);
        assert_eq!(o.terrain_cost(Terrain::Water), 1);
        assert_eq!(o.terrain_cost(Terrain::Sand), 3);
        assert_eq!(
            unit(Class::Geomancer, People::Undine).terrain_cost(Terrain::Sand),
            1
        );
        assert_eq!(
            unit(Class::Scout, People::Human).terrain_cost(Terrain::Sand),
            1
        );
        let d = unit(Class::Dragoon, People::Human);
        assert!(d.can_step(8, 0) && !d.can_step(0, 8));
    }

    #[test]
    fn mixed_peoples_battles_finish() {
        for seed in 0..20u64 {
            let mut s = setup(seed);
            for (i, u) in s.units.iter_mut().enumerate() {
                u.people = People::ALL[(i + seed as usize) % People::ALL.len()];
            }
            let (mut b, _) = Battle::new(&s);
            ai::autoplay(&mut b, 20_000);
            assert!(b.is_over());
        }
    }
}

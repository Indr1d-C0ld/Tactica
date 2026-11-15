use tactica_core::ability::AbilityId;
use tactica_core::ai;
use tactica_core::class::Class;
use tactica_core::map::{Biome, Map, MapParams};
use tactica_core::rng::Rng;
use tactica_core::statute::Statute;
use tactica_core::{Action, ActionError, Battle, BattleSetup, Event, Pos, UnitSetup};

fn setup(seed: u64) -> BattleSetup {
    let units = (0..2u8)
        .flat_map(|team| {
            Class::ALL.iter().map(move |&class| UnitSetup {
                name: format!("{team}-{}", class.name()),
                team,
                class,
                level: 3,
                xp: 0,
            })
        })
        .collect();
    BattleSetup {
        seed,
        map: None,
        units,
        statutes: 2,
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
        UnitSetup {
            name: "M".into(),
            team: 0,
            class: Class::Mender,
            level: 1,
            xp: 0,
        },
        UnitSetup {
            name: "X".into(),
            team: 1,
            class: Class::Vanguard,
            level: 1,
            xp: 0,
        },
    ];
    // Cerca un seed in cui lo statuto estratto è NoHealing e tocca al Guaritore.
    let (mut b, _) = (0..10_000)
        .map(|seed| {
            Battle::new(&BattleSetup {
                seed,
                map: Some(params.clone()),
                units: units.clone(),
                statutes: 1,
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

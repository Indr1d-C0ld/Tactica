//! Statistiche aggregate su molte battaglie IA contro IA (squadre speculari).
//! Uso: cargo run --release -p tactica-core --example balance [N]

use std::collections::BTreeMap;

use tactica_core::ai;
use tactica_core::class::Class;
use tactica_core::{Battle, BattleSetup, UnitSetup};

fn main() {
    let n: u64 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(1000);
    let mut wins = [0u32; 3];
    let mut turns = 0u64;
    let mut by_class: BTreeMap<&str, [i64; 4]> = BTreeMap::new(); // danni, cure, ko, sopravvissuti
    let mut by_biome: BTreeMap<&str, [u32; 3]> = BTreeMap::new();

    for seed in 0..n {
        let units = (0..2u8)
            .flat_map(|team| {
                Class::ALL.iter().map(move |&class| UnitSetup {
                    name: String::new(),
                    team,
                    class,
                    level: 3,
                    xp: 0,
                })
            })
            .collect();
        let (mut b, _) = Battle::new(&BattleSetup {
            seed,
            map: None,
            units,
            statutes: 2,
        });
        ai::autoplay(&mut b, 20_000);
        let w = match b.winner() {
            Some(Some(t)) => t as usize,
            _ => 2,
        };
        wins[w] += 1;
        by_biome.entry(b.map.biome.name()).or_default()[w] += 1;
        turns += b.turn() as u64;
        for u in &b.units {
            let e = by_class.entry(u.class.name()).or_default();
            e[0] += u.tally.damage_dealt as i64;
            e[1] += u.tally.healing_done as i64;
            e[2] += u.tally.kills as i64;
            e[3] += u.is_active() as i64;
        }
    }

    println!("Battaglie: {n}  turni medi: {:.1}", turns as f64 / n as f64);
    println!(
        "Vittorie squadra 0: {}  squadra 1: {}  pareggi: {}",
        wins[0], wins[1], wins[2]
    );
    println!(
        "\n{:<12} {:>10} {:>10} {:>8} {:>12}",
        "Classe", "Danni/bt", "Cure/bt", "KO/bt", "Soprav. %"
    );
    let per = (2 * n) as f64;
    for (c, v) in &by_class {
        println!(
            "{c:<12} {:>10.1} {:>10.1} {:>8.2} {:>11.1}%",
            v[0] as f64 / per,
            v[1] as f64 / per,
            v[2] as f64 / per,
            v[3] as f64 * 100.0 / per
        );
    }
    println!("\n{:<10} {:>6} {:>6} {:>6}", "Bioma", "Sq0", "Sq1", "Pari");
    for (b, v) in &by_biome {
        println!("{b:<10} {:>6} {:>6} {:>6}", v[0], v[1], v[2]);
    }
}

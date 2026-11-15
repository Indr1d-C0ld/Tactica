//! Bilanciamento delle classi: squadre casuali di 4 fra tutte le classi,
//! livello 12, scuole padroneggiate. Per ogni classe: % di vittorie delle
//! squadre che la schierano, danni e cure medi.
//! Uso: cargo run --release -p tactica-core --example classes [N]

use std::collections::BTreeMap;

use tactica_core::class::Class;
use tactica_core::people::People;
use tactica_core::rng::Rng;
use tactica_core::{Battle, BattleSetup, UnitSetup, ai, presets};

fn main() {
    let n: u64 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(600);
    // classe -> [presenze, vittorie, danni, cure, ko]
    let mut stats: BTreeMap<&str, [i64; 5]> = BTreeMap::new();
    let mut draws = 0;
    let mut by_people: BTreeMap<&str, [i64; 2]> = BTreeMap::new();
    for seed in 0..n {
        let mut rng = Rng::new(seed ^ 0xc1a5);
        let mut units = Vec::new();
        for team in 0..2u8 {
            for k in 0..4 {
                let c = Class::ALL[rng.below(Class::ALL.len() as u32) as usize];
                let mut u = UnitSetup::new(format!("{team}{k}"), team, c, 12)
                    .with_equipment(&presets::class_kit(c));
                u.mastered = c.learnable().to_vec();
                u.people = People::ALL[rng.below(People::ALL.len() as u32) as usize];
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
        ai::autoplay(&mut b, 30_000);
        let winner = b.winner().flatten();
        if winner.is_none() {
            draws += 1;
        }
        for u in &b.units {
            let e = stats.entry(u.class.name()).or_default();
            e[0] += 1;
            e[1] += (winner == Some(u.team)) as i64;
            e[2] += u.tally.damage_dealt as i64;
            e[3] += u.tally.healing_done as i64;
            e[4] += u.tally.kills as i64;
            let pe = by_people.entry(u.people.name()).or_default();
            pe[0] += 1;
            pe[1] += (winner == Some(u.team)) as i64;
        }
    }
    println!("Battaglie: {n}  pareggi: {draws}");
    println!(
        "{:<14} {:>8} {:>9} {:>9} {:>8}",
        "Classe", "Vitt. %", "Danni", "Cure", "KO"
    );
    let mut rows: Vec<_> = stats.into_iter().collect();
    rows.sort_by_key(|(_, e)| -(e[1] * 1000 / e[0].max(1)));
    for (name, e) in rows {
        let p = e[0].max(1);
        println!(
            "{name:<14} {:>7.1}% {:>9} {:>9} {:>8.2}",
            e[1] as f64 * 100.0 / p as f64,
            e[2] / p,
            e[3] / p,
            e[4] as f64 / p as f64
        );
    }
    println!("\n{:<10} {:>8}", "Popolo", "Vitt. %");
    for (name, e) in by_people {
        println!(
            "{name:<10} {:>7.1}%",
            e[1] as f64 * 100.0 / e[0].max(1) as f64
        );
    }
}

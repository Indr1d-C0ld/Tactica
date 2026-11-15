//! Demo: battaglia IA contro IA su mappa procedurale, con verifica del replay.
//!
//! Uso: tactica-cli [SEED] [--quiet]

use std::time::{SystemTime, UNIX_EPOCH};

use tactica_core::ai;
use tactica_core::presets;
use tactica_core::{Battle, BattleSetup, Event};

const NAMES: [[&str; 4]; 2] = [
    ["Alda", "Brenno", "Cora", "Dario"],
    ["Ezio", "Fosca", "Gualtiero", "Ilaria"],
];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let quiet = args.iter().any(|a| a == "--quiet" || a == "-q");
    let seed = args
        .iter()
        .find_map(|a| a.parse::<u64>().ok())
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(1, |d| d.as_nanos() as u64)
        });

    let setup = demo_setup(seed);
    let (mut b, start_events) = Battle::new(&setup);

    println!("Seed {seed}");
    println!(
        "Bioma: {} {}x{}",
        b.map.biome.name(),
        b.map.width,
        b.map.height
    );
    println!(
        "Statuti: {}",
        b.statutes
            .iter()
            .map(|s| s.name())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("{}", render(&b));
    println!("Legenda: . erba  # roccia  : sabbia  ~ acqua  ^ lava  \" boscaglia; cifra = altezza");
    println!("Squadra 0 = A-D, squadra 1 = a-d\n");

    let (actions, events) = ai::autoplay(&mut b, 10_000);
    if !quiet {
        for e in start_events.iter().chain(&events) {
            if let Some(line) = describe(&b, e) {
                println!("{line}");
            }
        }
        println!();
        println!("{}", render(&b));
    }

    match b.winner() {
        Some(Some(t)) => println!("Vince la squadra {t} al turno {}", b.turn()),
        Some(None) => println!("Pareggio al turno {}", b.turn()),
        None => println!("Interrotta al turno {}", b.turn()),
    }
    println!(
        "\n{:<10} {:<12} {:>3} {:>8} {:>6} {:>6} {:>6} {:>5} {:>5}",
        "Unità", "Classe", "Lv", "HP", "Danni", "Subiti", "Cure", "KO", "Viol"
    );
    for u in &b.units {
        println!(
            "{:<10} {:<12} {:>3} {:>4}/{:<3} {:>6} {:>6} {:>6} {:>5} {:>5}",
            u.name,
            u.class.name(),
            u.level,
            u.hp,
            u.stats.max_hp,
            u.tally.damage_dealt,
            u.tally.damage_taken,
            u.tally.healing_done,
            u.tally.kills,
            u.tally.violations
        );
    }

    // Replay: seed + azioni devono riprodurre esattamente lo stesso stato.
    let (mut replay, _) = Battle::new(&setup);
    for a in &actions {
        replay.apply(*a).expect("azione del replay rifiutata");
    }
    let (h1, h2) = (b.state_hash(), replay.state_hash());
    println!(
        "\nAzioni: {}  hash: {h1:016x}  replay: {}",
        actions.len(),
        if h1 == h2 { "identico" } else { "DIVERSO" }
    );
    if h1 != h2 {
        std::process::exit(1);
    }
}

fn demo_setup(seed: u64) -> BattleSetup {
    let mut units = presets::demo_party(0, 3, NAMES[0]);
    units.extend(presets::demo_party(1, 3, NAMES[1]));
    BattleSetup {
        seed,
        map: None,
        units,
        statutes: 2,
    }
}

fn letter(b: &Battle, id: usize) -> char {
    let u = &b.units[id];
    let i = b.units[..id].iter().filter(|o| o.team == u.team).count() as u8;
    if u.team == 0 {
        (b'A' + i) as char
    } else {
        (b'a' + i) as char
    }
}

fn render(b: &Battle) -> String {
    b.map.render(&|p| b.unit_at(p).map(|i| letter(b, i)))
}

fn describe(b: &Battle, e: &Event) -> Option<String> {
    tactica_core::text::describe_event(e, &|id| {
        let u = &b.units[id as usize];
        format!("{}({})", u.name, letter(b, id as usize))
    })
}

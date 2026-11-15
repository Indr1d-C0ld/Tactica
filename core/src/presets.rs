//! Squadre predefinite (demo, test, bilanciamento).

use crate::class::Class;
use crate::item::ItemId;
use crate::unit::UnitSetup;

/// Squadra di 4 classi base con equipaggiamento che insegna un'abilità ciascuno.
pub fn demo_party(team: u8, level: i32, names: [&str; 4]) -> Vec<UnitSetup> {
    use ItemId::*;
    let kits: [(Class, [ItemId; 2]); 4] = [
        (Class::Vanguard, [Greatblade, Chainmail]),
        (Class::Scout, [Longbow, Leather]),
        (Class::Arcanist, [StormRod, Robe]),
        (Class::Mender, [SunStaff, Robe]),
    ];
    kits.iter()
        .zip(names)
        .map(|((class, kit), name)| UnitSetup::new(name, team, *class, level).with_equipment(kit))
        .collect()
}

/// Equipaggiamento tipico per classe (nemici e squadre generate).
pub fn class_kit(class: Class) -> [ItemId; 2] {
    use ItemId::*;
    match class {
        Class::Vanguard => [Greatblade, Chainmail],
        Class::Scout => [Longbow, Leather],
        Class::Arcanist => [StormRod, Robe],
        Class::Mender => [SunStaff, Robe],
        Class::Runeblade => [RuneSaber, Chainmail],
        Class::Warden => [VotiveMace, WardenPlate],
    }
}

/// Missione cooperativa generata da seed: nemici scalati sulla squadra alleata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mission {
    pub name: String,
    pub difficulty: u8,
    /// Nemici (squadra 1).
    pub enemies: Vec<UnitSetup>,
    /// Indice del campione dentro `enemies`, se la missione è una caccia.
    pub champion: Option<usize>,
    pub statutes: usize,
}

pub const MAX_DIFFICULTY: u8 = 3;

pub fn difficulty_name(d: u8) -> &'static str {
    match d {
        1 => "Normale",
        2 => "Ardua",
        _ => "Leggendaria",
    }
}

const CHAMPIONS: [&str; 6] = [
    "Varek il Rosso",
    "Mordane",
    "Ysolde Senzavolto",
    "Guardiano di Cenere",
    "Thessa delle Spine",
    "Orvik Mezzaluna",
];
const GRUNTS: [&str; 8] = [
    "Predone",
    "Sentinella",
    "Veggente",
    "Eremita",
    "Lanciere",
    "Mercenario",
    "Strega",
    "Bruto",
];
const PLACES: [&str; 6] = [
    "della gola",
    "del guado",
    "delle rovine",
    "del passo",
    "della torre",
    "del bosco morto",
];

pub fn mission(seed: u64, allies: &[UnitSetup], difficulty: u8) -> Mission {
    let d = difficulty.clamp(1, MAX_DIFFICULTY);
    let mut rng = crate::rng::Rng::new(seed ^ 0x6d69_7373);
    let n = allies.len().max(1);
    let avg = allies.iter().map(|u| u.level).sum::<i32>() / n as i32;
    let level = (avg.max(1) + 2 * (d as i32 - 1)).clamp(1, crate::class::MAX_LEVEL);
    let count = (n.max(3) + d as usize - 1).min(8);
    let pool: &[Class] = if d >= 2 {
        &Class::ALL
    } else {
        &Class::ALL[..4]
    };
    let mut enemies: Vec<UnitSetup> = (0..count)
        .map(|i| {
            let class = pool[rng.below(pool.len() as u32) as usize];
            let name = format!(
                "{} {}",
                GRUNTS[rng.below(GRUNTS.len() as u32) as usize],
                i + 1
            );
            UnitSetup::new(name, 1, class, level).with_equipment(&class_kit(class))
        })
        .collect();
    let place = PLACES[rng.below(PLACES.len() as u32) as usize];
    let hunt = rng.percent(50);
    let (name, champion) = if hunt {
        let champ = CHAMPIONS[rng.below(CHAMPIONS.len() as u32) as usize];
        let e = &mut enemies[0];
        e.name = champ.to_string();
        e.level = (level + 3).min(crate::class::MAX_LEVEL);
        (format!("Caccia a {champ}"), Some(0))
    } else {
        (format!("Assedio {place}"), None)
    };
    Mission {
        name,
        difficulty: d,
        enemies,
        champion,
        statutes: if d >= 2 { 2 } else { 1 },
    }
}

/// Squadra che usa le abilità di status (test e bilanciamento).
pub fn status_party(team: u8, level: i32, names: [&str; 4]) -> Vec<UnitSetup> {
    use ItemId::*;
    let kits: [(Class, [ItemId; 2]); 4] = [
        (Class::Vanguard, [Greatblade, WarHorn]),
        (Class::Scout, [SerpentBow, Leather]),
        (Class::Arcanist, [QuietRod, Robe]),
        (Class::Mender, [SproutStaff, WhiteVestment]),
    ];
    kits.iter()
        .zip(names)
        .map(|((class, kit), name)| UnitSetup::new(name, team, *class, level).with_equipment(kit))
        .collect()
}

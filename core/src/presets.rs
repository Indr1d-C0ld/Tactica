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

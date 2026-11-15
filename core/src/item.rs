//! Equipaggiamento: bonus alle statistiche e abilità insegnate.

use crate::ability::AbilityId;
use crate::class::{Class, Stats};
use crate::gear::ItemRef;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Slot {
    Weapon,
    Armor,
    Accessory,
    Head,
}

impl Slot {
    pub const ALL: [Slot; 4] = [Slot::Weapon, Slot::Armor, Slot::Head, Slot::Accessory];

    pub fn name(self) -> &'static str {
        match self {
            Slot::Weapon => "arma",
            Slot::Armor => "armatura",
            Slot::Accessory => "accessorio",
            Slot::Head => "copricapo",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Slot::Weapon => "weapon",
            Slot::Armor => "armor",
            Slot::Accessory => "accessory",
            Slot::Head => "head",
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ItemId {
    IronSword,
    Greatblade,
    WarPike,
    Shortbow,
    Longbow,
    Hailbow,
    Rod,
    FrostRod,
    StormRod,
    CinderStaff,
    Staff,
    SunStaff,
    GraceStaff,
    RuneSaber,
    FrostSaber,
    VotiveMace,
    Leather,
    Chainmail,
    Robe,
    WardenPlate,
    SwiftBoots,
    Charm,
    SerpentBow,
    QuietRod,
    AegisPendant,
    SproutStaff,
    WhiteVestment,
    WarHorn,
    Flail,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item {
    pub id: ItemId,
    pub key: &'static str,
    pub name: &'static str,
    pub slot: Slot,
    /// Classi che possono equipaggiarlo (vuoto = tutte).
    pub classes: &'static [Class],
    pub bonus: Stats,
    pub teaches: Option<AbilityId>,
    pub price: i32,
    /// Acquistabile al negozio (altrimenti solo bottino).
    pub in_shop: bool,
}

const fn bonus(atk: i32, def: i32, mag: i32, res: i32, hp: i32, mp: i32) -> Stats {
    Stats {
        atk,
        def,
        mag,
        res,
        max_hp: hp,
        max_mp: mp,
        ..Stats::ZERO
    }
}

const ALL_CLASSES: &[Class] = &[];

macro_rules! item {
    ($id:ident, $key:literal, $name:literal, $slot:ident, $classes:expr, $bonus:expr, $teaches:expr, $price:literal, $shop:literal) => {
        Item {
            id: ItemId::$id,
            key: $key,
            name: $name,
            slot: Slot::$slot,
            classes: $classes,
            bonus: $bonus,
            teaches: $teaches,
            price: $price,
            in_shop: $shop,
        }
    };
}

use AbilityId as A;
use Class::*;

/// Indicizzata per `ItemId as usize`.
static TABLE: [Item; 29] = [
    item!(
        IronSword,
        "iron_sword",
        "Spada di ferro",
        Weapon,
        &[Vanguard, Runeblade],
        bonus(3, 0, 0, 0, 0, 0),
        None,
        100,
        true
    ),
    item!(
        Greatblade,
        "greatblade",
        "Spadone",
        Weapon,
        &[Vanguard],
        bonus(6, 0, 0, 0, 0, 0),
        Some(A::Sweep),
        400,
        true
    ),
    item!(
        WarPike,
        "war_pike",
        "Picca da guerra",
        Weapon,
        &[Vanguard, Warden],
        bonus(5, 0, 0, 0, 0, 0),
        Some(A::Charge),
        450,
        true
    ),
    item!(
        Shortbow,
        "shortbow",
        "Arco corto",
        Weapon,
        &[Scout],
        bonus(2, 0, 0, 0, 0, 0),
        None,
        100,
        true
    ),
    item!(
        Longbow,
        "longbow",
        "Arco lungo",
        Weapon,
        &[Scout],
        bonus(5, 0, 0, 0, 0, 0),
        Some(A::AimedShot),
        380,
        true
    ),
    item!(
        Hailbow,
        "hailbow",
        "Arco grandine",
        Weapon,
        &[Scout],
        Stats {
            atk: 4,
            spd: 1,
            ..Stats::ZERO
        },
        Some(A::Volley),
        520,
        true
    ),
    item!(
        Rod,
        "rod",
        "Verga",
        Weapon,
        &[Arcanist],
        bonus(0, 0, 2, 0, 0, 0),
        None,
        100,
        true
    ),
    item!(
        FrostRod,
        "frost_rod",
        "Verga di brina",
        Weapon,
        &[Arcanist],
        bonus(0, 0, 4, 0, 0, 0),
        Some(A::Frost),
        350,
        true
    ),
    item!(
        StormRod,
        "storm_rod",
        "Verga di tempesta",
        Weapon,
        &[Arcanist],
        bonus(0, 0, 5, 0, 0, 0),
        Some(A::Bolt),
        480,
        true
    ),
    item!(
        CinderStaff,
        "cinder_staff",
        "Bastone di brace",
        Weapon,
        &[Arcanist],
        bonus(0, 0, 7, 0, 0, 0),
        Some(A::Inferno),
        700,
        false
    ),
    item!(
        Staff,
        "staff",
        "Bastone",
        Weapon,
        &[Mender, Warden],
        bonus(0, 0, 2, 0, 0, 0),
        None,
        100,
        true
    ),
    item!(
        SunStaff,
        "sun_staff",
        "Bastone solare",
        Weapon,
        &[Mender],
        bonus(0, 0, 4, 0, 0, 0),
        Some(A::Radiance),
        360,
        true
    ),
    item!(
        GraceStaff,
        "grace_staff",
        "Bastone di grazia",
        Weapon,
        &[Mender],
        bonus(0, 0, 5, 2, 0, 0),
        Some(A::GreatMend),
        600,
        false
    ),
    item!(
        RuneSaber,
        "rune_saber",
        "Sciabola runica",
        Weapon,
        &[Runeblade],
        bonus(5, 0, 3, 0, 0, 0),
        Some(A::DrainBlade),
        550,
        true
    ),
    item!(
        FrostSaber,
        "frost_saber",
        "Sciabola gelida",
        Weapon,
        &[Runeblade],
        bonus(6, 0, 2, 0, 0, 0),
        Some(A::FrostEdge),
        600,
        false
    ),
    item!(
        VotiveMace,
        "votive_mace",
        "Mazza votiva",
        Weapon,
        &[Warden],
        bonus(4, 0, 3, 0, 0, 0),
        Some(A::Judgment),
        600,
        true
    ),
    item!(
        Leather,
        "leather",
        "Giaco di cuoio",
        Armor,
        ALL_CLASSES,
        bonus(0, 2, 0, 0, 10, 0),
        None,
        80,
        true
    ),
    item!(
        Chainmail,
        "chainmail",
        "Cotta di maglia",
        Armor,
        &[Vanguard, Runeblade, Warden],
        Stats {
            def: 5,
            max_hp: 20,
            spd: -1,
            ..Stats::ZERO
        },
        None,
        300,
        true
    ),
    item!(
        Robe,
        "robe",
        "Tunica",
        Armor,
        &[Arcanist, Mender, Runeblade, Warden],
        bonus(0, 0, 0, 3, 0, 10),
        None,
        120,
        true
    ),
    item!(
        WardenPlate,
        "warden_plate",
        "Corazza del custode",
        Armor,
        &[Warden],
        bonus(0, 6, 0, 3, 15, 0),
        Some(A::Sanctuary),
        650,
        false
    ),
    item!(
        SwiftBoots,
        "swift_boots",
        "Stivali lesti",
        Accessory,
        ALL_CLASSES,
        Stats {
            mov: 1,
            ..Stats::ZERO
        },
        None,
        500,
        true
    ),
    item!(
        Charm,
        "charm",
        "Amuleto",
        Accessory,
        ALL_CLASSES,
        Stats {
            evade: 5,
            res: 1,
            ..Stats::ZERO
        },
        None,
        250,
        true
    ),
    item!(
        SerpentBow,
        "serpent_bow",
        "Arco del serpente",
        Weapon,
        &[Scout],
        bonus(4, 0, 0, 0, 0, 0),
        Some(A::VenomDart),
        460,
        true
    ),
    item!(
        QuietRod,
        "quiet_rod",
        "Verga della quiete",
        Weapon,
        &[Arcanist],
        bonus(0, 0, 4, 1, 0, 0),
        Some(A::Torpor),
        520,
        true
    ),
    item!(
        AegisPendant,
        "aegis_pendant",
        "Pendaglio dell'egida",
        Accessory,
        ALL_CLASSES,
        bonus(0, 1, 0, 1, 0, 0),
        Some(A::Aegis),
        380,
        true
    ),
    item!(
        SproutStaff,
        "sprout_staff",
        "Bastone del germoglio",
        Weapon,
        &[Mender],
        bonus(0, 0, 4, 0, 0, 5),
        Some(A::Sprout),
        480,
        true
    ),
    item!(
        WhiteVestment,
        "white_vestment",
        "Veste candida",
        Armor,
        &[Mender, Warden, Arcanist],
        bonus(0, 1, 0, 4, 0, 8),
        Some(A::Purify),
        420,
        true
    ),
    item!(
        WarHorn,
        "war_horn",
        "Corno da battaglia",
        Accessory,
        ALL_CLASSES,
        Stats {
            spd: 1,
            ..Stats::ZERO
        },
        Some(A::Rally),
        520,
        true
    ),
    item!(
        Flail,
        "flail",
        "Flagello",
        Weapon,
        &[Warden],
        bonus(5, 0, 1, 0, 0, 0),
        Some(A::Chains),
        540,
        true
    ),
];

impl ItemId {
    pub const ALL: [ItemId; 29] = {
        let mut out = [ItemId::IronSword; 29];
        let mut i = 0;
        while i < 29 {
            out[i] = TABLE[i].id;
            i += 1;
        }
        out
    };

    pub fn def(self) -> &'static Item {
        &TABLE[self as usize]
    }

    pub fn key(self) -> &'static str {
        self.def().key
    }

    pub fn from_key(key: &str) -> Option<ItemId> {
        Self::ALL.into_iter().find(|i| i.key() == key)
    }
}

impl Item {
    pub fn usable_by(&self, class: Class) -> bool {
        self.classes.is_empty() || self.classes.contains(&class)
    }
}

/// Equipaggiamento iniziale di una nuova recluta.
pub fn starter_kit(class: Class) -> [ItemId; 2] {
    let weapon = match class {
        Class::Vanguard | Class::Runeblade => ItemId::IronSword,
        Class::Scout => ItemId::Shortbow,
        Class::Arcanist => ItemId::Rod,
        Class::Mender | Class::Warden => ItemId::Staff,
    };
    [weapon, ItemId::Leather]
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadoutError {
    NotForClass(String),
    SlotTaken(Slot),
}

impl std::fmt::Display for LoadoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadoutError::NotForClass(name) => {
                write!(f, "{name} non è utilizzabile da questa classe")
            }
            LoadoutError::SlotTaken(s) => write!(f, "uno solo oggetto per slot ({})", s.name()),
        }
    }
}

/// Al massimo un oggetto per slot, tutti utilizzabili dalla classe.
pub fn validate_loadout(class: Class, equipment: &[ItemRef]) -> Result<(), LoadoutError> {
    for (i, it) in equipment.iter().enumerate() {
        if !it.usable_by(class) {
            return Err(LoadoutError::NotForClass(it.name()));
        }
        if equipment[..i].iter().any(|o| o.slot() == it.slot()) {
            return Err(LoadoutError::SlotTaken(it.slot()));
        }
    }
    Ok(())
}

/// Tiene solo la parte valida di un equipaggiamento (primo oggetto per slot).
pub fn sanitize_loadout(class: Class, equipment: &[ItemRef]) -> Vec<ItemRef> {
    let mut out: Vec<ItemRef> = Vec::new();
    for &it in equipment {
        if it.usable_by(class) && !out.iter().any(|o| o.slot() == it.slot()) {
            out.push(it);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_matches_enum() {
        for (i, it) in TABLE.iter().enumerate() {
            assert_eq!(it.id as usize, i, "{}", it.name);
            assert_eq!(ItemId::from_key(it.key), Some(it.id));
            if let Some(a) = it.teaches {
                let school = a.school().expect("abilità di scuola");
                assert!(school.learnable().contains(&a));
                assert!(
                    it.usable_by(school),
                    "{} deve essere usabile dalla scuola",
                    it.name
                );
            }
        }
        for c in Class::ALL {
            assert!(validate_loadout(c, &starter_kit(c).map(ItemRef::from)).is_ok());
            // Ogni abilità imparabile ha almeno un oggetto che la insegna.
            for a in c.learnable() {
                assert!(TABLE.iter().any(|i| i.teaches == Some(*a)), "{a:?}");
            }
        }
    }
}

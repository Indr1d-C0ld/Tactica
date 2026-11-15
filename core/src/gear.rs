//! Oggetti generati: rarità, basi, materiali, affissi e leggendari.
//!
//! Un oggetto generato è descritto per intero dalla sua chiave testuale
//! (`g.spa.2.r.aff.lup.-.sweep`), quindi inventario, equipaggiamento e
//! replay lo salvano come una stringa qualsiasi. Gli oggetti del catalogo
//! classico (`ItemId`) convivono con quelli generati tramite `ItemRef`.

use crate::ability::{AbilityId, Element};
use crate::class::{Class, Stats};
use crate::consumable::Consumable;
use crate::item::{ItemId, Slot};
use crate::rng::Rng;
use crate::status::{Status, StatusEffect};

// ------------------------------------------------------------------ rarità

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

impl Rarity {
    pub const ALL: [Rarity; 5] = [
        Rarity::Common,
        Rarity::Uncommon,
        Rarity::Rare,
        Rarity::Epic,
        Rarity::Legendary,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Rarity::Common => "comune",
            Rarity::Uncommon => "non comune",
            Rarity::Rare => "raro",
            Rarity::Epic => "epico",
            Rarity::Legendary => "leggendario",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Rarity::Common => "common",
            Rarity::Uncommon => "uncommon",
            Rarity::Rare => "rare",
            Rarity::Epic => "epic",
            Rarity::Legendary => "legendary",
        }
    }

    fn code(self) -> char {
        match self {
            Rarity::Common => 'c',
            Rarity::Uncommon => 'n',
            Rarity::Rare => 'r',
            Rarity::Epic => 'e',
            Rarity::Legendary => 'l',
        }
    }

    fn from_code(c: &str) -> Option<Rarity> {
        Rarity::ALL
            .into_iter()
            .find(|r| c.len() == 1 && c.starts_with(r.code()))
    }

    /// Moltiplicatore del prezzo (in centesimi).
    fn price_pct(self) -> i32 {
        match self {
            Rarity::Common => 100,
            Rarity::Uncommon => 180,
            Rarity::Rare => 300,
            Rarity::Epic => 520,
            Rarity::Legendary => 1000,
        }
    }
}

// ------------------------------------------------------------------ effetti

/// Effetto speciale di un oggetto, oltre alle statistiche.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Effect {
    /// Colpi fisici: probabilità di infliggere uno status.
    OnHit(Status, i32, i32),
    /// Colpi fisici: recupera questa % del danno inflitto.
    Drain(i32),
    /// Gli attacchi fisici senza elemento assumono questo elemento.
    Element(Element),
    /// Danni di questo elemento dimezzati.
    Resist(Element),
    /// Recupera questa % degli HP massimi a fine turno.
    Regen(i32),
    /// Immune a questo status.
    Immune(Status),
}

impl Effect {
    pub fn describe(self) -> String {
        match self {
            Effect::OnHit(s, c, d) => format!("{}% di {} al colpo ({} turni)", c, s.name(), d),
            Effect::Drain(p) => format!("assorbe il {p}% dei danni inflitti"),
            Effect::Element(e) => format!("attacchi di {}", e.name()),
            Effect::Resist(e) => format!("resiste a {} (danni dimezzati)", e.name()),
            Effect::Regen(p) => format!("rigenera il {p}% degli HP a fine turno"),
            Effect::Immune(s) => format!("immune a {}", s.name()),
        }
    }

    /// Gli effetti al colpo crescono un po' con la qualità del materiale.
    fn at_tier(self, tier: u8) -> Effect {
        match self {
            Effect::OnHit(s, c, d) => Effect::OnHit(s, c + 2 * tier as i32, d),
            Effect::Drain(p) => Effect::Drain(p + tier as i32),
            e => e,
        }
    }
}

/// Effetti di tutto l'equipaggiamento di un'unità.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GearEffects {
    pub on_hit: Vec<StatusEffect>,
    pub drain_pct: i32,
    pub attack_element: Option<Element>,
    pub resist: Vec<Element>,
    pub regen_pct: i32,
    pub immune: Vec<Status>,
}

impl GearEffects {
    pub fn add(&mut self, e: Effect) {
        match e {
            Effect::OnHit(status, chance, duration) => self.on_hit.push(StatusEffect {
                status,
                chance,
                duration,
            }),
            Effect::Drain(p) => self.drain_pct += p,
            Effect::Element(el) => {
                self.attack_element.get_or_insert(el);
            }
            Effect::Resist(el) => self.resist.push(el),
            Effect::Regen(p) => self.regen_pct += p,
            Effect::Immune(s) => self.immune.push(s),
        }
    }
}

// ------------------------------------------------------------------ basi

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Metal,
    Wood,
    Cloth,
    Leather,
    Jewel,
    Paper,
}

impl Family {
    /// Materiali dal più comune al più prezioso (uno per livello di qualità).
    fn materials(self) -> [&'static str; TIERS] {
        match self {
            Family::Metal => [
                "di bronzo",
                "di ferro",
                "d'acciaio",
                "d'argento runico",
                "d'oricalco",
                "di ferro stellare",
            ],
            Family::Wood => [
                "di frassino",
                "di tasso",
                "di quercia nera",
                "d'ebano",
                "di legno lunare",
                "di radice antica",
            ],
            Family::Cloth => [
                "di lino",
                "di lana",
                "di seta",
                "di seta lunare",
                "di filo d'ombra",
                "di tessuto astrale",
            ],
            Family::Leather => [
                "di cuoio grezzo",
                "di cuoio conciato",
                "di pelle di cervo",
                "di pelle di basilisco",
                "di pelle di viverna",
                "di scaglie di drago",
            ],
            Family::Jewel => [
                "di rame",
                "d'argento",
                "d'oro",
                "di platino",
                "di cristallo",
                "di stellite",
            ],
            Family::Paper => [
                "di papiro",
                "di pergamena",
                "di vello",
                "d'avorio",
                "di giada",
                "di luce fossile",
            ],
        }
    }
}

pub const TIERS: usize = 6;
/// Moltiplicatore delle statistiche per livello di qualità (in centesimi).
const TIER_PCT: [i32; TIERS] = [100, 150, 210, 280, 360, 450];

/// Livello di qualità del materiale per il livello di chi lascia l'oggetto.
pub fn tier_for_level(level: i32) -> u8 {
    match level {
        ..=4 => 0,
        5..=9 => 1,
        10..=17 => 2,
        18..=27 => 3,
        28..=39 => 4,
        _ => 5,
    }
}

pub struct Base {
    pub code: &'static str,
    pub noun: &'static str,
    /// Sostantivo femminile (per l'accordo degli aggettivi).
    pub fem: bool,
    pub slot: Slot,
    /// Classi che possono usarlo (vuoto = tutte).
    pub classes: &'static [Class],
    pub family: Family,
    /// Statistiche al primo livello di qualità.
    pub stats: Stats,
}

#[allow(clippy::too_many_arguments)]
const fn st(
    atk: i32,
    def: i32,
    mag: i32,
    res: i32,
    hp: i32,
    mp: i32,
    spd: i32,
    evade: i32,
) -> Stats {
    Stats {
        max_hp: hp,
        max_mp: mp,
        atk,
        def,
        mag,
        res,
        spd,
        mov: 0,
        jump: 0,
        evade,
    }
}

const fn mj(mov: i32, jump: i32) -> Stats {
    Stats {
        mov,
        jump,
        ..Stats::ZERO
    }
}

use Class::{
    Alchemist as Al, Arcanist as A, Bard as Bd, Dragoon as Dr, Geomancer as Ge, Mender as M,
    Runeblade as R, Scout as S, Shadowblade as Sh, Summoner as Su, Vanguard as V, Warden as W,
};
use Family::*;
use Slot::{Accessory as Acc, Armor as Arm, Head, Weapon as Wpn};

macro_rules! base {
    ($code:literal, $noun:literal, $fem:literal, $slot:ident, [$($c:ident),*], $fam:ident, $stats:expr) => {
        Base { code: $code, noun: $noun, fem: $fem, slot: $slot, classes: &[$($c),*], family: $fam, stats: $stats }
    };
}

pub static BASES: [Base; 37] = [
    // Armi
    base!(
        "spa",
        "Spada",
        true,
        Wpn,
        [V, R, Sh],
        Metal,
        st(3, 0, 0, 0, 0, 0, 0, 0)
    ),
    base!(
        "sgr",
        "Spadone",
        false,
        Wpn,
        [V],
        Metal,
        st(5, 0, 0, 0, 0, 0, -1, 0)
    ),
    base!(
        "lan",
        "Lancia",
        true,
        Wpn,
        [V, W, Dr],
        Metal,
        st(4, 0, 0, 0, 0, 0, 0, 0)
    ),
    base!(
        "asc",
        "Ascia",
        true,
        Wpn,
        [V, W, Dr, Ge],
        Metal,
        st(5, 0, 0, 0, 0, 0, 0, -3)
    ),
    base!(
        "maz",
        "Mazza",
        true,
        Wpn,
        [M, W, Ge, Al],
        Metal,
        st(3, 0, 0, 1, 0, 0, 0, 0)
    ),
    base!(
        "pug",
        "Pugnale",
        false,
        Wpn,
        [S, R, Sh, Bd],
        Metal,
        st(2, 0, 0, 0, 0, 0, 0, 5)
    ),
    base!(
        "sci",
        "Sciabola",
        true,
        Wpn,
        [R, S, Sh],
        Metal,
        st(3, 0, 1, 0, 0, 0, 0, 0)
    ),
    base!(
        "arc",
        "Arco corto",
        false,
        Wpn,
        [S, Bd],
        Wood,
        st(3, 0, 0, 0, 0, 0, 0, 0)
    ),
    base!(
        "arl",
        "Arco lungo",
        false,
        Wpn,
        [S],
        Wood,
        st(4, 0, 0, 0, 0, 0, 0, 0)
    ),
    base!(
        "bal",
        "Balestra",
        true,
        Wpn,
        [S],
        Wood,
        st(5, 0, 0, 0, 0, 0, -1, 0)
    ),
    base!(
        "bac",
        "Bacchetta",
        true,
        Wpn,
        [A, Al, Su],
        Wood,
        st(0, 0, 3, 0, 0, 0, 0, 0)
    ),
    base!(
        "ver",
        "Verga",
        true,
        Wpn,
        [A, Su],
        Metal,
        st(0, 0, 4, 0, 0, 4, 0, 0)
    ),
    base!(
        "bas",
        "Bastone",
        false,
        Wpn,
        [M, A, W, Al, Ge, Su, Bd],
        Wood,
        st(0, 0, 2, 2, 0, 0, 0, 0)
    ),
    base!(
        "lru",
        "Lama runica",
        true,
        Wpn,
        [R],
        Metal,
        st(3, 0, 2, 0, 0, 0, 0, 0)
    ),
    base!(
        "fla",
        "Flagello",
        false,
        Wpn,
        [W],
        Metal,
        st(4, 0, 1, 0, 0, 0, 0, 0)
    ),
    base!(
        "tom",
        "Tomo",
        false,
        Wpn,
        [A, M, Al, Su],
        Paper,
        st(0, 0, 3, 1, 0, 6, 0, 0)
    ),
    // Corazze
    base!(
        "tun",
        "Tunica",
        true,
        Arm,
        [],
        Cloth,
        st(0, 0, 0, 0, 8, 0, 0, 0)
    ),
    base!(
        "ves",
        "Veste",
        true,
        Arm,
        [A, M, Bd, Al, Su],
        Cloth,
        st(0, 0, 0, 2, 0, 5, 0, 0)
    ),
    base!(
        "sai",
        "Saio",
        false,
        Arm,
        [M, W, Ge, Al, Su],
        Cloth,
        st(0, 0, 0, 2, 5, 0, 0, 0)
    ),
    base!(
        "giu",
        "Giubba",
        true,
        Arm,
        [S, R, V, Sh, Bd, Al, Ge],
        Leather,
        st(0, 1, 0, 0, 5, 0, 0, 0)
    ),
    base!(
        "gia",
        "Giaco",
        false,
        Arm,
        [S, R, Sh, Bd],
        Leather,
        st(0, 2, 0, 0, 0, 0, 0, 3)
    ),
    base!(
        "usb",
        "Usbergo",
        false,
        Arm,
        [V, R, W, Dr, Ge],
        Metal,
        st(0, 3, 0, 0, 0, 0, 0, 0)
    ),
    base!(
        "bri",
        "Brigantina",
        true,
        Arm,
        [V, S, R, Dr, Sh],
        Metal,
        st(0, 3, 0, 0, 4, 0, 0, 0)
    ),
    base!(
        "cpi",
        "Corazza",
        true,
        Arm,
        [V, W, Dr],
        Metal,
        st(0, 5, 0, 0, 0, 0, -1, 0)
    ),
    // Copricapi
    base!(
        "elm",
        "Elmo",
        false,
        Head,
        [V, W, R, Dr, Ge],
        Metal,
        st(0, 2, 0, 0, 4, 0, 0, 0)
    ),
    base!(
        "bar",
        "Barbuta",
        true,
        Head,
        [V, W, Dr],
        Metal,
        st(0, 3, 0, 0, 0, 0, 0, -2)
    ),
    base!(
        "cpc",
        "Cappuccio",
        false,
        Head,
        [S, M, A, Sh, Bd, Al, Ge],
        Cloth,
        st(0, 0, 0, 1, 0, 0, 0, 2)
    ),
    base!(
        "cpl",
        "Cappello",
        false,
        Head,
        [A, M, Bd, Al, Su],
        Cloth,
        st(0, 0, 1, 0, 0, 4, 0, 0)
    ),
    base!(
        "cer",
        "Cerchietto",
        false,
        Head,
        [],
        Jewel,
        st(0, 0, 1, 1, 0, 0, 0, 0)
    ),
    base!(
        "ben",
        "Benda",
        true,
        Head,
        [],
        Cloth,
        st(0, 0, 0, 0, 3, 0, 0, 2)
    ),
    // Accessori
    base!(
        "ane",
        "Anello",
        false,
        Acc,
        [],
        Jewel,
        st(1, 0, 1, 0, 0, 0, 0, 0)
    ),
    base!(
        "amu",
        "Amuleto",
        false,
        Acc,
        [],
        Jewel,
        st(0, 0, 0, 2, 0, 0, 0, 0)
    ),
    base!(
        "cin",
        "Cintura",
        true,
        Acc,
        [],
        Leather,
        st(0, 0, 0, 0, 6, 0, 0, 0)
    ),
    base!(
        "brc",
        "Bracciale",
        false,
        Acc,
        [],
        Jewel,
        st(1, 1, 0, 0, 0, 0, 0, 0)
    ),
    base!(
        "man",
        "Mantello",
        false,
        Acc,
        [],
        Cloth,
        st(0, 0, 0, 1, 0, 0, 0, 3)
    ),
    base!(
        "spl",
        "Spilla",
        true,
        Acc,
        [],
        Jewel,
        st(0, 0, 0, 0, 0, 5, 0, 0)
    ),
    base!(
        "tal",
        "Talismano",
        false,
        Acc,
        [],
        Jewel,
        st(0, 0, 0, 1, 4, 0, 0, 0)
    ),
];

fn base_index(code: &str) -> Option<u8> {
    BASES.iter().position(|b| b.code == code).map(|i| i as u8)
}

impl Base {
    pub fn usable_by(&self, class: Class) -> bool {
        self.classes.is_empty() || self.classes.contains(&class)
    }
}

// ------------------------------------------------------------------ affissi

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    /// Aggettivo dopo il materiale (maschile, femminile).
    Prefix(&'static str, &'static str),
    /// Complemento invariabile ("del Lupo").
    Suffix(&'static str),
}

pub struct Affix {
    pub code: &'static str,
    pub form: Form,
    /// Slot su cui può comparire.
    pub slots: &'static [Slot],
    /// Statistiche al primo livello di qualità (HP, MP e statistiche principali crescono col materiale).
    pub stats: Stats,
    pub effect: Option<Effect>,
    /// Rarità minima.
    pub min: Rarity,
}

const ANY: &[Slot] = &[Wpn, Arm, Head, Acc];
const WPN: &[Slot] = &[Wpn];
const DEF: &[Slot] = &[Arm, Head, Acc];
const HA: &[Slot] = &[Head, Acc];

macro_rules! affix {
    ($code:literal, pre($m:literal, $f:literal), $slots:expr, $stats:expr, $eff:expr, $min:ident) => {
        Affix {
            code: $code,
            form: Form::Prefix($m, $f),
            slots: $slots,
            stats: $stats,
            effect: $eff,
            min: Rarity::$min,
        }
    };
    ($code:literal, suf($s:literal), $slots:expr, $stats:expr, $eff:expr, $min:ident) => {
        Affix {
            code: $code,
            form: Form::Suffix($s),
            slots: $slots,
            stats: $stats,
            effect: $eff,
            min: Rarity::$min,
        }
    };
}

const Z: Stats = Stats::ZERO;
use Effect as E;

pub static AFFIXES: [Affix; 39] = [
    // Aggettivi: statistiche
    affix!(
        "aff",
        pre("affilato", "affilata"),
        WPN,
        st(2, 0, 0, 0, 0, 0, 0, 0),
        None,
        Uncommon
    ),
    affix!(
        "pes",
        pre("pesante", "pesante"),
        WPN,
        st(3, 0, 0, 0, 0, 0, -1, 0),
        None,
        Uncommon
    ),
    affix!(
        "arc",
        pre("arcano", "arcana"),
        &[Wpn, Head, Acc],
        st(0, 0, 2, 0, 0, 0, 0, 0),
        None,
        Uncommon
    ),
    affix!(
        "rob",
        pre("robusto", "robusta"),
        DEF,
        st(0, 0, 0, 0, 6, 0, 0, 0),
        None,
        Uncommon
    ),
    affix!(
        "sal",
        pre("saldo", "salda"),
        &[Arm, Head],
        st(0, 2, 0, 0, 0, 0, 0, 0),
        None,
        Uncommon
    ),
    affix!(
        "ben",
        pre("benedetto", "benedetta"),
        DEF,
        st(0, 0, 0, 2, 0, 0, 0, 0),
        None,
        Uncommon
    ),
    affix!(
        "mis",
        pre("mistico", "mistica"),
        ANY,
        st(0, 0, 0, 0, 0, 5, 0, 0),
        None,
        Uncommon
    ),
    affix!(
        "rap",
        pre("rapido", "rapida"),
        &[Wpn, Head, Acc],
        st(0, 0, 0, 0, 0, 0, 1, 0),
        None,
        Rare
    ),
    affix!(
        "sfu",
        pre("sfuggente", "sfuggente"),
        DEF,
        st(0, 0, 0, 0, 0, 0, 0, 6),
        None,
        Uncommon
    ),
    affix!(
        "equ",
        pre("equilibrato", "equilibrata"),
        WPN,
        st(1, 1, 1, 1, 0, 0, 0, 0),
        None,
        Uncommon
    ),
    affix!(
        "ant",
        pre("antico", "antica"),
        ANY,
        st(1, 1, 1, 1, 4, 2, 0, 0),
        None,
        Epic
    ),
    // Aggettivi: effetti
    affix!(
        "ven",
        pre("velenoso", "velenosa"),
        WPN,
        Z,
        Some(E::OnHit(Status::Poison, 20, 3)),
        Uncommon
    ),
    affix!(
        "vam",
        pre("vampirico", "vampirica"),
        WPN,
        Z,
        Some(E::Drain(15)),
        Rare
    ),
    affix!(
        "fia",
        pre("fiammeggiante", "fiammeggiante"),
        WPN,
        Z,
        Some(E::Element(Element::Fire)),
        Uncommon
    ),
    affix!(
        "gel",
        pre("gelido", "gelida"),
        WPN,
        Z,
        Some(E::Element(Element::Ice)),
        Uncommon
    ),
    affix!(
        "fol",
        pre("folgorante", "folgorante"),
        WPN,
        Z,
        Some(E::Element(Element::Thunder)),
        Uncommon
    ),
    affix!(
        "sac",
        pre("sacro", "sacra"),
        WPN,
        Z,
        Some(E::Element(Element::Holy)),
        Rare
    ),
    affix!(
        "rig",
        pre("rigenerante", "rigenerante"),
        DEF,
        Z,
        Some(E::Regen(4)),
        Rare
    ),
    // Complementi: statistiche
    affix!(
        "lup",
        suf("del Lupo"),
        ANY,
        st(2, 0, 0, 0, 4, 0, 0, 0),
        None,
        Uncommon
    ),
    affix!(
        "ors",
        suf("dell'Orso"),
        DEF,
        st(0, 1, 0, 0, 8, 0, 0, 0),
        None,
        Uncommon
    ),
    affix!(
        "vol",
        suf("della Volpe"),
        ANY,
        st(0, 0, 0, 0, 0, 0, 1, 4),
        None,
        Rare
    ),
    affix!(
        "guf",
        suf("del Gufo"),
        ANY,
        st(0, 0, 2, 0, 0, 4, 0, 0),
        None,
        Uncommon
    ),
    affix!(
        "que",
        suf("della Quercia"),
        DEF,
        st(0, 2, 0, 2, 0, 0, 0, 0),
        None,
        Uncommon
    ),
    affix!(
        "fal",
        suf("del Falco"),
        &[Wpn, Acc],
        st(1, 0, 0, 0, 0, 0, 1, 0),
        None,
        Rare
    ),
    affix!(
        "ser",
        suf("della Serpe"),
        WPN,
        st(1, 0, 0, 0, 0, 0, 0, 3),
        None,
        Uncommon
    ),
    affix!("via", suf("del Viandante"), HA, mj(1, 0), None, Epic),
    affix!("stm", suf("dello Stambecco"), HA, mj(0, 1), None, Rare),
    affix!(
        "fen",
        suf("della Fenice"),
        DEF,
        st(0, 0, 0, 1, 5, 0, 0, 0),
        Some(E::Regen(3)),
        Epic
    ),
    // Complementi: resistenze e immunità
    affix!(
        "slm",
        suf("della Salamandra"),
        DEF,
        Z,
        Some(E::Resist(Element::Fire)),
        Uncommon
    ),
    affix!(
        "brn",
        suf("della Brina"),
        DEF,
        Z,
        Some(E::Resist(Element::Ice)),
        Uncommon
    ),
    affix!(
        "tmp",
        suf("della Tempesta"),
        DEF,
        Z,
        Some(E::Resist(Element::Thunder)),
        Uncommon
    ),
    affix!(
        "alb",
        suf("dell'Alba"),
        DEF,
        Z,
        Some(E::Resist(Element::Holy)),
        Rare
    ),
    affix!(
        "veg",
        suf("della Veglia"),
        HA,
        Z,
        Some(E::Immune(Status::Sleep)),
        Rare
    ),
    affix!(
        "voc",
        suf("della Voce"),
        HA,
        Z,
        Some(E::Immune(Status::Silence)),
        Rare
    ),
    affix!(
        "pur",
        suf("della Purezza"),
        DEF,
        Z,
        Some(E::Immune(Status::Poison)),
        Rare
    ),
    // Complementi: effetti al colpo
    affix!(
        "rov",
        suf("dei Rovi"),
        WPN,
        Z,
        Some(E::OnHit(Status::Root, 15, 2)),
        Rare
    ),
    affix!(
        "son",
        suf("del Sonno"),
        WPN,
        Z,
        Some(E::OnHit(Status::Sleep, 10, 2)),
        Epic
    ),
    affix!(
        "sil",
        suf("del Silenzio"),
        WPN,
        Z,
        Some(E::OnHit(Status::Silence, 15, 2)),
        Rare
    ),
    affix!(
        "gls",
        suf("del Gelo"),
        WPN,
        Z,
        Some(E::OnHit(Status::Slow, 20, 3)),
        Uncommon
    ),
];

fn affix_index(code: &str) -> Option<u8> {
    AFFIXES.iter().position(|a| a.code == code).map(|i| i as u8)
}

// ------------------------------------------------------------------ leggendari

pub struct Legend {
    pub code: &'static str,
    pub name: &'static str,
    pub base: &'static str,
    /// Statistiche in più rispetto alla base (crescono col materiale).
    pub stats: Stats,
    pub effects: &'static [Effect],
    pub teaches: Option<AbilityId>,
    pub lore: &'static str,
}

macro_rules! legend {
    ($code:literal, $name:literal, $base:literal, $stats:expr, [$($e:expr),*], $teach:expr, $lore:literal) => {
        Legend { code: $code, name: $name, base: $base, stats: $stats, effects: &[$($e),*], teaches: $teach, lore: $lore }
    };
}

use AbilityId as Ab;

pub static LEGENDS: [Legend; 16] = [
    legend!(
        "alba",
        "Lama dell'Alba",
        "spa",
        st(4, 0, 0, 2, 0, 0, 0, 0),
        [E::Element(Element::Holy), E::Regen(4)],
        None,
        "Forgiata il giorno in cui fu scritto il Patto."
    ),
    legend!(
        "zanna",
        "Zanna di Velmara",
        "pug",
        st(3, 0, 0, 0, 0, 0, 1, 6),
        [E::OnHit(Status::Poison, 35, 3)],
        Some(Ab::VenomDart),
        "Un dente di serpe incastonato nell'argento."
    ),
    legend!(
        "bastione",
        "Bastione dei Custodi",
        "cpi",
        st(0, 4, 0, 3, 10, 0, 0, 0),
        [E::Immune(Status::Sleep), E::Resist(Element::Holy)],
        None,
        "Ha retto la Sala dei Custodi per tre assedi."
    ),
    legend!(
        "sigillo",
        "Sigillo del Patto",
        "amu",
        st(0, 0, 0, 3, 0, 8, 0, 0),
        [E::Immune(Status::Silence), E::Immune(Status::Sleep)],
        None,
        "Chi lo porta non può essere zittito."
    ),
    legend!(
        "brace",
        "Arco di Brace",
        "arl",
        st(3, 0, 0, 0, 0, 0, 0, 0),
        [E::Element(Element::Fire)],
        Some(Ab::Volley),
        "Le frecce si accendono appena lasciano la corda."
    ),
    legend!(
        "nevi",
        "Verga delle Nevi Eterne",
        "ver",
        st(0, 0, 5, 0, 0, 0, 0, 0),
        [E::Element(Element::Ice), E::OnHit(Status::Slow, 30, 3)],
        Some(Ab::Frost),
        "Non si è mai sciolta, nemmeno a Bocca di Brace."
    ),
    legend!(
        "tuono",
        "Tuono di Nebbiaforte",
        "fla",
        st(4, 0, 1, 0, 0, 0, 0, 0),
        [E::Element(Element::Thunder), E::OnHit(Status::Root, 20, 2)],
        Some(Ab::Chains),
        "Ogni colpo rimbomba come la palude in tempesta."
    ),
    legend!(
        "radici",
        "Corona di Radici",
        "cer",
        st(0, 0, 2, 2, 0, 0, 0, 0),
        [E::Regen(6), E::Immune(Status::Poison)],
        None,
        "Germoglia ancora, a ogni primavera."
    ),
    legend!(
        "grigio",
        "Mantello del Viandante Grigio",
        "man",
        st(0, 0, 0, 0, 0, 0, 1, 8),
        [E::Resist(Element::Ice)],
        None,
        "Appartenne a chi attraversò Velmara senza mai fermarsi."
    ),
    legend!(
        "sorgente",
        "Veste della Sorgente",
        "ves",
        st(0, 0, 2, 3, 0, 10, 0, 0),
        [E::Regen(4)],
        Some(Ab::GreatMend),
        "Tessuta con l'acqua della sorgente di Lunaverde."
    ),
    legend!(
        "fauce",
        "Fauce del Vulcano",
        "asc",
        st(6, 0, 0, 0, 0, 0, 0, 0),
        [E::Element(Element::Fire), E::Drain(20)],
        None,
        "Ancora calda dalle forge della Cenere."
    ),
    legend!(
        "cielo",
        "Elmo del Cielo Spezzato",
        "elm",
        st(0, 3, 0, 0, 8, 0, 0, 0),
        [E::Resist(Element::Thunder)],
        None,
        "Una crepa lo attraversa: il fulmine che lo colpì non l'ha piegato."
    ),
    legend!(
        "eco",
        "Eco di Varen",
        "lru",
        st(3, 0, 3, 0, 0, 0, 0, 0),
        [E::Drain(15), E::OnHit(Status::Silence, 20, 2)],
        Some(Ab::DrainBlade),
        "La lama dell'ultimo Custode caduto."
    ),
    legend!(
        "lunaverde",
        "Anello di Lunaverde",
        "ane",
        st(2, 0, 2, 0, 0, 0, 1, 0),
        [],
        None,
        "Un semplice anello di campagna che non perde mai lucentezza."
    ),
    legend!(
        "sabbie",
        "Cintura delle Sabbie",
        "cin",
        st(0, 0, 0, 0, 10, 0, 0, 0),
        [E::Resist(Element::Fire), E::Immune(Status::Poison)],
        None,
        "Dono dei carovanieri di Sabbiarossa."
    ),
    legend!(
        "cenere",
        "Grimorio della Cenere",
        "tom",
        st(0, 0, 5, 0, 0, 8, 0, 0),
        [E::Element(Element::Fire)],
        Some(Ab::Inferno),
        "Le sue pagine bruciano senza consumarsi."
    ),
];

fn legend_index(code: &str) -> Option<u8> {
    LEGENDS.iter().position(|l| l.code == code).map(|i| i as u8)
}

// ------------------------------------------------------------------ oggetto generato

const NONE: u8 = u8::MAX;

/// Un oggetto generato: tutto ciò che serve a ricostruirlo sta nella chiave.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Gear {
    pub base: u8,
    pub tier: u8,
    pub rarity: Rarity,
    pub affixes: [u8; 3],
    pub teaches: Option<AbilityId>,
    /// Indice del leggendario (solo per rarità leggendaria).
    pub legend: u8,
}

fn scale(s: Stats, tier: u8) -> Stats {
    let p = TIER_PCT[tier as usize];
    let f = |v: i32| {
        if v == 0 {
            0
        } else {
            (v * p + 50 * v.signum()) / 100
        }
    };
    Stats {
        max_hp: f(s.max_hp),
        max_mp: f(s.max_mp),
        atk: f(s.atk),
        def: f(s.def),
        mag: f(s.mag),
        res: f(s.res),
        // Velocità, movimento, salto e schivata non crescono col materiale.
        spd: s.spd,
        mov: s.mov,
        jump: s.jump,
        evade: s.evade,
    }
}

impl Gear {
    pub fn base(&self) -> &'static Base {
        &BASES[self.base as usize]
    }

    pub fn legend(&self) -> Option<&'static Legend> {
        (self.legend != NONE).then(|| &LEGENDS[self.legend as usize])
    }

    pub fn affixes(&self) -> impl Iterator<Item = &'static Affix> + '_ {
        self.affixes
            .iter()
            .filter(|&&a| a != NONE)
            .map(|&a| &AFFIXES[a as usize])
    }

    pub fn key(&self) -> String {
        let tier = self.tier;
        if let Some(l) = self.legend() {
            return format!("u.{}.{tier}", l.code);
        }
        let aff: Vec<&str> = (0..3)
            .map(|i| match self.affixes[i] {
                NONE => "-",
                a => AFFIXES[a as usize].code,
            })
            .collect();
        format!(
            "g.{}.{tier}.{}.{}.{}",
            self.base().code,
            self.rarity.code(),
            aff.join("."),
            self.teaches.map_or("-", |a| a.key())
        )
    }

    pub fn from_key(key: &str) -> Option<Gear> {
        let p: Vec<&str> = key.split('.').collect();
        match p.as_slice() {
            ["u", code, tier] => {
                let li = legend_index(code)?;
                let tier: u8 = tier.parse().ok().filter(|&t: &u8| (t as usize) < TIERS)?;
                let l = &LEGENDS[li as usize];
                Some(Gear {
                    base: base_index(l.base)?,
                    tier,
                    rarity: Rarity::Legendary,
                    affixes: [NONE; 3],
                    teaches: l.teaches,
                    legend: li,
                })
            }
            ["g", base, tier, rar, a1, a2, a3, teach] => {
                let base = base_index(base)?;
                let tier: u8 = tier.parse().ok().filter(|&t: &u8| (t as usize) < TIERS)?;
                let rarity = Rarity::from_code(rar).filter(|r| *r != Rarity::Legendary)?;
                let mut affixes = [NONE; 3];
                for (i, a) in [a1, a2, a3].into_iter().enumerate() {
                    if *a != "-" {
                        affixes[i] = affix_index(a)?;
                    }
                }
                let teaches = match *teach {
                    "-" => None,
                    k => Some(AbilityId::from_key(k)?),
                };
                Some(Gear {
                    base,
                    tier,
                    rarity,
                    affixes,
                    teaches,
                    legend: NONE,
                })
            }
            _ => None,
        }
    }

    pub fn name(&self) -> String {
        if let Some(l) = self.legend() {
            return l.name.to_string();
        }
        let b = self.base();
        let mut name = format!("{} {}", b.noun, b.family.materials()[self.tier as usize]);
        let mut suffixes = Vec::new();
        for a in self.affixes() {
            match a.form {
                Form::Prefix(m, f) => {
                    name.push(' ');
                    name.push_str(if b.fem { f } else { m });
                }
                Form::Suffix(s) => suffixes.push(s),
            }
        }
        if !suffixes.is_empty() {
            name.push(' ');
            name.push_str(&suffixes.join(" e "));
        }
        name
    }

    pub fn bonus(&self) -> Stats {
        let mut s = self.base().stats;
        if let Some(l) = self.legend() {
            s = s.plus(l.stats);
        }
        for a in self.affixes() {
            s = s.plus(a.stats);
        }
        let mut s = scale(s, self.tier);
        // Le rarità più alte rendono un po' di più anche le statistiche di base.
        let q = match self.rarity {
            Rarity::Epic => 110,
            Rarity::Legendary => 120,
            _ => 100,
        };
        for v in [
            &mut s.max_hp,
            &mut s.max_mp,
            &mut s.atk,
            &mut s.def,
            &mut s.mag,
            &mut s.res,
        ] {
            *v = *v * q / 100;
        }
        s
    }

    pub fn effects(&self) -> Vec<Effect> {
        let mut v: Vec<Effect> = self.affixes().filter_map(|a| a.effect).collect();
        if let Some(l) = self.legend() {
            v.extend_from_slice(l.effects);
        }
        v.into_iter().map(|e| e.at_tier(self.tier)).collect()
    }

    pub fn price(&self) -> i32 {
        let slot = match self.base().slot {
            Slot::Weapon => 120,
            Slot::Armor => 100,
            Slot::Head => 80,
            Slot::Accessory | Slot::Bag => 90,
        };
        let teach = if self.teaches.is_some() { 150 } else { 100 };
        slot * TIER_PCT[self.tier as usize] / 100 * self.rarity.price_pct() / 100 * teach / 100
    }
}

// ------------------------------------------------------------------ riferimento a un oggetto

/// Un oggetto qualsiasi: del catalogo classico o generato.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ItemRef {
    Fixed(ItemId),
    Gen(Gear),
    /// Consumabile da borsa (pergamena, pozione...).
    Use(Consumable),
}

impl From<ItemId> for ItemRef {
    fn from(i: ItemId) -> Self {
        ItemRef::Fixed(i)
    }
}

impl ItemRef {
    pub fn key(&self) -> String {
        match self {
            ItemRef::Fixed(i) => i.key().to_string(),
            ItemRef::Gen(g) => g.key(),
            ItemRef::Use(c) => c.key(),
        }
    }

    pub fn from_key(key: &str) -> Option<ItemRef> {
        ItemId::from_key(key)
            .map(ItemRef::Fixed)
            .or_else(|| Gear::from_key(key).map(ItemRef::Gen))
            .or_else(|| Consumable::from_key(key).map(ItemRef::Use))
    }

    pub fn name(&self) -> String {
        match self {
            ItemRef::Fixed(i) => i.def().name.to_string(),
            ItemRef::Gen(g) => g.name(),
            ItemRef::Use(c) => c.name(),
        }
    }

    pub fn slot(&self) -> Slot {
        match self {
            ItemRef::Fixed(i) => i.def().slot,
            ItemRef::Gen(g) => g.base().slot,
            ItemRef::Use(_) => Slot::Bag,
        }
    }

    pub fn classes(&self) -> &'static [Class] {
        match self {
            ItemRef::Fixed(i) => i.def().classes,
            ItemRef::Gen(g) => g.base().classes,
            ItemRef::Use(_) => &[],
        }
    }

    pub fn usable_by(&self, class: Class) -> bool {
        let c = self.classes();
        c.is_empty() || c.contains(&class)
    }

    pub fn bonus(&self) -> Stats {
        match self {
            ItemRef::Fixed(i) => i.def().bonus,
            ItemRef::Gen(g) => g.bonus(),
            ItemRef::Use(_) => Stats::ZERO,
        }
    }

    pub fn teaches(&self) -> Option<AbilityId> {
        match self {
            ItemRef::Fixed(i) => i.def().teaches,
            ItemRef::Gen(g) => g.teaches,
            ItemRef::Use(_) => None,
        }
    }

    pub fn effects(&self) -> Vec<Effect> {
        match self {
            ItemRef::Fixed(_) | ItemRef::Use(_) => Vec::new(),
            ItemRef::Gen(g) => g.effects(),
        }
    }

    pub fn price(&self) -> i32 {
        match self {
            ItemRef::Fixed(i) => i.def().price,
            ItemRef::Gen(g) => g.price(),
            ItemRef::Use(c) => c.price(),
        }
    }

    pub fn rarity(&self) -> Rarity {
        match self {
            ItemRef::Fixed(i) => {
                let d = i.def();
                match (d.in_shop, d.price) {
                    (true, _) => Rarity::Common,
                    (false, ..=500) => Rarity::Uncommon,
                    _ => Rarity::Rare,
                }
            }
            ItemRef::Gen(g) => g.rarity,
            ItemRef::Use(c) => c.rarity(),
        }
    }

    /// Nome dell'icona: la base per gli oggetti generati, la chiave per gli altri.
    pub fn icon(&self) -> String {
        match self {
            ItemRef::Fixed(i) => fixed_icon(*i).to_string(),
            ItemRef::Gen(g) => match g.legend() {
                Some(l) => format!("legend_{}", l.code),
                None => g.base().code.to_string(),
            },
            ItemRef::Use(c) => format!("use_{}", c.def().code),
        }
    }

    /// Testo descrittivo per l'interfaccia (lore dei leggendari).
    pub fn lore(&self) -> Option<&'static str> {
        match self {
            ItemRef::Gen(g) => g.legend().map(|l| l.lore),
            ItemRef::Fixed(_) | ItemRef::Use(_) => None,
        }
    }

    /// Il consumabile, se lo è.
    pub fn consumable(&self) -> Option<Consumable> {
        match self {
            ItemRef::Use(c) => Some(*c),
            _ => None,
        }
    }
}

/// Icona degli oggetti del catalogo classico: la base più simile.
fn fixed_icon(i: ItemId) -> &'static str {
    use ItemId::*;
    match i {
        IronSword => "spa",
        Greatblade => "sgr",
        WarPike => "lan",
        Shortbow | SerpentBow => "arc",
        Longbow => "arl",
        Hailbow => "bal",
        Rod | QuietRod => "bac",
        FrostRod | StormRod => "ver",
        CinderStaff | Staff | SunStaff | GraceStaff | SproutStaff => "bas",
        RuneSaber | FrostSaber => "lru",
        VotiveMace => "maz",
        Leather => "gia",
        Chainmail => "usb",
        Robe | WhiteVestment => "ves",
        WardenPlate => "cpi",
        SwiftBoots => "boots",
        Charm => "tal",
        AegisPendant => "amu",
        WarHorn => "horn",
        Flail => "fla",
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for ItemRef {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.key())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ItemRef {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        // Compatibile con i vecchi salvataggi, che usavano i nomi delle varianti di ItemId.
        let s = String::deserialize(d)?;
        ItemRef::from_key(&s)
            .or_else(|| {
                ItemId::ALL
                    .into_iter()
                    .find(|i| format!("{i:?}") == s)
                    .map(ItemRef::Fixed)
            })
            .ok_or_else(|| serde::de::Error::custom(format!("oggetto sconosciuto: {s}")))
    }
}

// ------------------------------------------------------------------ generazione

/// Oggetto comune di una base (per i kit delle classi senza oggetti del catalogo).
pub fn common(base_code: &str, tier: u8) -> Option<ItemRef> {
    Some(ItemRef::Gen(Gear {
        base: base_index(base_code)?,
        tier: tier.min(TIERS as u8 - 1),
        rarity: Rarity::Common,
        affixes: [NONE; 3],
        teaches: None,
        legend: NONE,
    }))
}

/// Rarità estratta: `boost` (0-10) sposta il peso verso le rarità alte.
pub fn roll_rarity(rng: &mut Rng, boost: i32) -> Rarity {
    let b = boost.clamp(0, 10);
    let weights = [
        (600 - 45 * b).max(120),
        260 + 6 * b,
        100 + 14 * b,
        32 + 9 * b,
        8 + 4 * b,
    ];
    let total: i32 = weights.iter().sum();
    let mut roll = rng.below(total as u32) as i32;
    for (i, w) in weights.iter().enumerate() {
        if roll < *w {
            return Rarity::ALL[i];
        }
        roll -= w;
    }
    Rarity::Common
}

/// Probabilità (%) che un oggetto del bottino sia un consumabile.
pub const CONSUMABLE_PCT: i32 = 32;

/// Oggetto casuale per chi lascia il bottino a questo livello.
pub fn roll_item(rng: &mut Rng, level: i32, boost: i32) -> ItemRef {
    let rarity = roll_rarity(rng, boost);
    let mut tier = tier_for_level(level) as i32;
    if rng.percent(12) {
        tier += 1;
    }
    let tier = tier.clamp(0, TIERS as i32 - 1) as u8;
    // Circa un oggetto su tre è un consumabile (pergamena, pozione, bomba...).
    if rng.percent(CONSUMABLE_PCT) {
        return ItemRef::Use(crate::consumable::roll(rng, rarity, tier));
    }
    if rarity == Rarity::Legendary {
        let li = rng.below(LEGENDS.len() as u32) as u8;
        let l = &LEGENDS[li as usize];
        return ItemRef::Gen(Gear {
            base: base_index(l.base).unwrap(),
            tier,
            rarity,
            affixes: [NONE; 3],
            teaches: l.teaches,
            legend: li,
        });
    }
    let base = rng.below(BASES.len() as u32) as u8;
    let slot = BASES[base as usize].slot;
    let count = match rarity {
        Rarity::Common => 0,
        Rarity::Uncommon => 1,
        Rarity::Rare => 2,
        _ => 3,
    };
    let mut affixes = [NONE; 3];
    let mut n = 0;
    let mut tries = 0;
    while n < count && tries < 60 {
        tries += 1;
        let a = rng.below(AFFIXES.len() as u32) as u8;
        let def = &AFFIXES[a as usize];
        let is_prefix = matches!(def.form, Form::Prefix(..));
        let clash = affixes[..n].iter().any(|&o| {
            o == a
                || (is_prefix && matches!(AFFIXES[o as usize].form, Form::Prefix(..)))
                || (def.effect.is_some()
                    && AFFIXES[o as usize].effect.is_some()
                    && std::mem::discriminant(&def.effect.unwrap())
                        == std::mem::discriminant(&AFFIXES[o as usize].effect.unwrap()))
        });
        if def.slots.contains(&slot) && def.min <= rarity && !clash {
            affixes[n] = a;
            n += 1;
        }
    }
    // Prima gli aggettivi, poi i complementi, in ordine stabile.
    affixes[..n].sort_by_key(|&a| (matches!(AFFIXES[a as usize].form, Form::Suffix(_)), a));
    let teach_pct = match rarity {
        Rarity::Rare => 30,
        Rarity::Epic => 65,
        _ => 0,
    };
    let teaches = if rng.percent(teach_pct) {
        let b = &BASES[base as usize];
        let pool: Vec<AbilityId> = Class::ALL
            .into_iter()
            .filter(|c| b.usable_by(*c))
            .flat_map(|c| c.learnable().iter().copied())
            .collect();
        (!pool.is_empty()).then(|| pool[rng.below(pool.len() as u32) as usize])
    } else {
        None
    };
    ItemRef::Gen(Gear {
        base,
        tier,
        rarity,
        affixes,
        teaches,
        legend: NONE,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_roundtrip_and_names_read_well() {
        let mut rng = Rng::new(7);
        let mut seen = std::collections::HashSet::new();
        for i in 0..5000 {
            let it = roll_item(&mut rng, (i % 50) + 1, i % 11);
            let key = it.key();
            assert_eq!(ItemRef::from_key(&key), Some(it), "{key}");
            assert!(!it.name().contains("  "));
            if let Some(a) = it.teaches() {
                let school = a.school().unwrap();
                assert!(
                    it.usable_by(school),
                    "{key}: insegna {a:?} a chi non può usarlo"
                );
            }
            seen.insert(key);
        }
        // Varietà: migliaia di oggetti diversi anche in poche estrazioni
        // (i comuni si ripetono spesso, come è giusto).
        assert!(seen.len() > 2000, "solo {} oggetti diversi", seen.len());
    }

    #[test]
    fn tables_are_consistent() {
        for (i, b) in BASES.iter().enumerate() {
            assert_eq!(base_index(b.code), Some(i as u8));
        }
        for (i, a) in AFFIXES.iter().enumerate() {
            assert_eq!(affix_index(a.code), Some(i as u8));
        }
        for l in &LEGENDS {
            let b = &BASES[base_index(l.base).expect("base del leggendario") as usize];
            if let Some(a) = l.teaches {
                assert!(b.usable_by(a.school().unwrap()), "{}", l.name);
            }
        }
    }

    #[test]
    fn rarity_boost_shifts_toward_rare() {
        let count = |boost| {
            let mut rng = Rng::new(1);
            (0..4000)
                .filter(|_| roll_rarity(&mut rng, boost) >= Rarity::Rare)
                .count()
        };
        assert!(count(8) > count(0) * 2);
    }

    #[test]
    fn higher_tiers_are_stronger() {
        let g = |tier| Gear {
            base: base_index("spa").unwrap(),
            tier,
            rarity: Rarity::Common,
            affixes: [NONE; 3],
            teaches: None,
            legend: NONE,
        };
        assert!(g(5).bonus().atk > g(0).bonus().atk * 3);
        assert_eq!(g(2).name(), "Spada d'acciaio");
    }
}

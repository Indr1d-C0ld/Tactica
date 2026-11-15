//! Tabella delle abilità.
//!
//! Ogni classe ha abilità innate e abilità della propria "scuola" che si usano
//! solo finché un oggetto equipaggiato le insegna, oppure per sempre una volta
//! padroneggiate accumulando punti risonanza (vedi `progression`).

use crate::class::Class;
use crate::status::{Status, StatusEffect};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Physical,
    Magic,
    Heal,
    /// Nessun danno né cura: applica uno status o ne rimuove.
    Support,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Element {
    Fire,
    Ice,
    Thunder,
    Holy,
}

impl Element {
    pub fn name(self) -> &'static str {
        match self {
            Element::Fire => "fuoco",
            Element::Ice => "ghiaccio",
            Element::Thunder => "fulmine",
            Element::Holy => "luce",
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AbilityId {
    Strike,
    HeavyBlow,
    Sweep,
    Charge,
    Shot,
    AimedShot,
    Volley,
    Flame,
    Frost,
    Bolt,
    Inferno,
    Mend,
    Radiance,
    GreatMend,
    RuneEdge,
    DrainBlade,
    FrostEdge,
    Rebuke,
    Sanctuary,
    Judgment,
    VenomDart,
    Torpor,
    Aegis,
    Sprout,
    Purify,
    Rally,
    Chains,
    // Seconda serie di abilità di scuola.
    LegSweep,
    Bulwark,
    Cleave,
    PiercingArrow,
    HushArrow,
    BrambleSnare,
    Blizzard,
    Thunderstorm,
    ArcaneHush,
    Meteor,
    Prayer,
    HolyRay,
    Return,
    FlameEdge,
    StormEdge,
    RuneSeal,
    ShieldWall,
    LightHammer,
    Vow,
    // Classi avanzate della terza serie.
    Backstab,
    SleepBlade,
    VenomFang,
    BladeDance,
    ShadowVeil,
    Leap,
    DiveStrike,
    DragonLance,
    Roar,
    Ballad,
    WarSong,
    Lullaby,
    Dirge,
    Hymn,
    Lob,
    Blastmix,
    ToxicFumes,
    Panacea,
    Tonic,
    Landslide,
    Quicksand,
    Earthquake,
    Stoneskin,
    Sap,
    Spark,
    FireSpirit,
    FrostSpirit,
    ThunderSpirit,
    DawnSpirit,
    // Effetti degli oggetti consumabili (non si imparano).
    Potion,
    Ether,
    Remedy,
    Revive,
    Elixir,
    FireBomb,
    FrostVial,
    ToxinVial,
    SleepPowder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ability {
    pub id: AbilityId,
    /// Chiave stabile per database e protocollo.
    pub key: &'static str,
    pub name: &'static str,
    pub kind: Kind,
    pub power: i32,
    pub mp_cost: i32,
    pub range_min: i32,
    pub range_max: i32,
    /// Raggio dell'area (0 = bersaglio singolo).
    pub aoe: i32,
    /// Dislivello massimo tra chi agisce e la casella bersaglio.
    pub vertical: i32,
    pub element: Option<Element>,
    pub ranged: bool,
    /// Chi colpisce recupera metà dei danni inflitti.
    pub drain: bool,
    /// Punti risonanza per padroneggiarla (0 = innata, non si impara).
    pub master_rp: i32,
    /// Status inflitto (ai nemici se negativo, agli alleati se positivo).
    pub effect: Option<StatusEffect>,
    /// Rimuove gli status negativi dagli alleati colpiti.
    pub cleanse: bool,
    /// Valore fisso (`power`), indipendente dalle statistiche di chi agisce.
    pub fixed: bool,
    /// La cura ripristina MP invece di HP.
    pub restore_mp: bool,
    /// Rianima un alleato sconfitto con `power`% degli HP.
    pub revive: bool,
}

const BASE: Ability = Ability {
    id: AbilityId::Strike,
    key: "",
    name: "",
    kind: Kind::Physical,
    power: 100,
    mp_cost: 0,
    range_min: 1,
    range_max: 1,
    aoe: 0,
    vertical: 2,
    element: None,
    ranged: false,
    drain: false,
    master_rp: 0,
    effect: None,
    cleanse: false,
    fixed: false,
    restore_mp: false,
    revive: false,
};

const fn fx(status: Status, chance: i32, duration: i32) -> Option<StatusEffect> {
    Some(StatusEffect {
        status,
        chance,
        duration,
    })
}

macro_rules! ab {
    ($id:ident, $key:literal, $name:literal, $kind:ident, $power:literal, $mp:literal, ($rmin:literal, $rmax:literal) $(, $f:ident: $v:expr)*) => {
        Ability {
            id: AbilityId::$id,
            key: $key,
            name: $name,
            kind: Kind::$kind,
            power: $power,
            mp_cost: $mp,
            range_min: $rmin,
            range_max: $rmax,
            $($f: $v,)*
            ..BASE
        }
    };
}

use Element::*;

/// Indicizzata per `AbilityId as usize`: l'ordine deve coincidere con l'enum.
static TABLE: [Ability; 84] = [
    ab!(Strike, "strike", "Colpo", Physical, 100, 0, (1, 1)),
    ab!(
        HeavyBlow,
        "heavy_blow",
        "Fendente",
        Physical,
        160,
        8,
        (1, 1)
    ),
    ab!(Sweep, "sweep", "Ampio fendente", Physical, 90, 6, (1, 1), aoe: 1, master_rp: 80),
    ab!(Charge, "charge", "Carica", Physical, 130, 6, (1, 2), master_rp: 100),
    ab!(Shot, "shot", "Tiro", Physical, 90, 0, (2, 5), vertical: 5, ranged: true),
    ab!(AimedShot, "aimed_shot", "Tiro mirato", Physical, 140, 8, (2, 6), vertical: 6, ranged: true, master_rp: 80),
    ab!(Volley, "volley", "Raffica", Physical, 70, 10, (3, 5), aoe: 1, vertical: 5, ranged: true, master_rp: 120),
    ab!(Flame, "flame", "Fiamma", Magic, 100, 8, (0, 4), aoe: 1, vertical: 4, element: Some(Fire)),
    ab!(Frost, "frost", "Gelo", Magic, 110, 8, (0, 4), aoe: 1, vertical: 4, element: Some(Ice), master_rp: 80,
        effect: fx(Status::Slow, 30, 3)),
    ab!(Bolt, "bolt", "Folgore", Magic, 160, 14, (1, 4), vertical: 6, element: Some(Thunder), master_rp: 100,
        effect: fx(Status::Root, 25, 2)),
    ab!(Inferno, "inferno", "Vampa", Magic, 150, 18, (2, 4), aoe: 1, vertical: 4, element: Some(Fire), master_rp: 150),
    ab!(Mend, "mend", "Cura", Heal, 100, 6, (0, 3), aoe: 1, vertical: 4),
    ab!(Radiance, "radiance", "Luce", Magic, 90, 6, (1, 3), vertical: 4, element: Some(Holy), master_rp: 80),
    ab!(GreatMend, "great_mend", "Cura maggiore", Heal, 140, 14, (0, 3), aoe: 1, vertical: 4, master_rp: 120),
    ab!(RuneEdge, "rune_edge", "Lama runica", Physical, 120, 4, (1, 1), element: Some(Thunder)),
    ab!(DrainBlade, "drain_blade", "Lama vorace", Physical, 110, 6, (1, 1), drain: true, master_rp: 100),
    ab!(FrostEdge, "frost_edge", "Taglio gelido", Physical, 130, 8, (1, 1), element: Some(Ice), master_rp: 100,
        effect: fx(Status::Slow, 35, 3)),
    ab!(Rebuke, "rebuke", "Ammonimento", Magic, 95, 4, (1, 3), vertical: 4, element: Some(Holy),
        effect: fx(Status::Silence, 40, 3)),
    ab!(Sanctuary, "sanctuary", "Santuario", Heal, 60, 12, (0, 2), aoe: 2, vertical: 4, master_rp: 120),
    ab!(Judgment, "judgment", "Giudizio", Magic, 140, 16, (1, 4), vertical: 6, element: Some(Holy), master_rp: 140),
    ab!(VenomDart, "venom_dart", "Dardo velenoso", Physical, 70, 6, (2, 5), vertical: 5, ranged: true, master_rp: 90,
        effect: fx(Status::Poison, 80, 4)),
    ab!(Torpor, "torpor", "Torpore", Support, 0, 10, (1, 4), aoe: 1, vertical: 4, master_rp: 110,
        effect: fx(Status::Sleep, 70, 2)),
    ab!(Aegis, "aegis", "Egida", Support, 0, 6, (0, 3), vertical: 4, master_rp: 80,
        effect: fx(Status::Aegis, 100, 3)),
    ab!(Sprout, "sprout", "Germoglio", Support, 0, 8, (0, 3), aoe: 1, vertical: 4, master_rp: 100,
        effect: fx(Status::Regen, 100, 3)),
    ab!(Purify, "purify", "Purificazione", Support, 0, 6, (0, 3), aoe: 1, vertical: 4, master_rp: 90, cleanse: true),
    ab!(Rally, "rally", "Slancio", Support, 0, 8, (0, 0), aoe: 1, vertical: 4, master_rp: 110,
        effect: fx(Status::Haste, 100, 3)),
    ab!(Chains, "chains", "Catene", Magic, 50, 8, (1, 3), vertical: 4, master_rp: 100,
        effect: fx(Status::Root, 75, 2)),
    // Seconda serie: Avanguardia
    ab!(LegSweep, "leg_sweep", "Colpo alle gambe", Physical, 90, 6, (1, 1), master_rp: 90,
        effect: fx(Status::Root, 55, 2)),
    ab!(Bulwark, "bulwark", "Baluardo", Support, 0, 8, (0, 0), aoe: 1, vertical: 4, master_rp: 110,
        effect: fx(Status::Aegis, 100, 2)),
    ab!(Cleave, "cleave", "Squarcio", Physical, 170, 9, (1, 1), master_rp: 140),
    // Esploratore
    ab!(PiercingArrow, "piercing_arrow", "Freccia perforante", Physical, 120, 5, (2, 7), vertical: 7,
        ranged: true, master_rp: 100),
    ab!(HushArrow, "hush_arrow", "Freccia del silenzio", Physical, 80, 8, (2, 5), vertical: 5,
        ranged: true, master_rp: 100, effect: fx(Status::Silence, 55, 2)),
    ab!(BrambleSnare, "bramble_snare", "Trappola di rovi", Support, 0, 8, (1, 4), aoe: 1, vertical: 4,
        ranged: true, master_rp: 110, effect: fx(Status::Root, 70, 2)),
    // Arcanista
    ab!(Blizzard, "blizzard", "Bufera", Magic, 115, 20, (2, 5), aoe: 2, vertical: 5, element: Some(Ice),
        master_rp: 150, effect: fx(Status::Slow, 35, 2)),
    ab!(Thunderstorm, "thunderstorm", "Temporale", Magic, 120, 16, (1, 4), aoe: 1, vertical: 6,
        element: Some(Thunder), master_rp: 130),
    ab!(ArcaneHush, "arcane_hush", "Silenzio arcano", Support, 0, 10, (1, 4), aoe: 1, vertical: 4,
        master_rp: 110, effect: fx(Status::Silence, 65, 2)),
    ab!(Meteor, "meteor", "Meteora", Magic, 200, 28, (3, 5), aoe: 1, vertical: 6, element: Some(Fire),
        master_rp: 180),
    // Guaritore
    ab!(Prayer, "prayer", "Preghiera", Heal, 75, 18, (0, 2), aoe: 2, vertical: 4, master_rp: 130),
    ab!(HolyRay, "holy_ray", "Raggio sacro", Magic, 130, 12, (1, 4), vertical: 5, element: Some(Holy),
        master_rp: 110),
    ab!(Return, "return", "Ritorno", Heal, 25, 20, (1, 2), vertical: 4, revive: true, master_rp: 160),
    // Spadaruna
    ab!(FlameEdge, "flame_edge", "Lama di fiamma", Physical, 120, 6, (1, 1), element: Some(Fire),
        master_rp: 90),
    ab!(StormEdge, "storm_edge", "Lama tonante", Physical, 115, 8, (1, 1), element: Some(Thunder),
        master_rp: 110, effect: fx(Status::Root, 30, 2)),
    ab!(RuneSeal, "rune_seal", "Sigillo runico", Magic, 70, 10, (1, 3), vertical: 4, master_rp: 110,
        effect: fx(Status::Silence, 60, 2)),
    // Custode
    ab!(ShieldWall, "shield_wall", "Muro di scudi", Support, 0, 14, (0, 0), aoe: 2, vertical: 4,
        master_rp: 140, effect: fx(Status::Aegis, 100, 2)),
    ab!(LightHammer, "light_hammer", "Martello di luce", Physical, 160, 8, (1, 1), element: Some(Holy),
        master_rp: 110),
    ab!(Vow, "vow", "Voto", Support, 0, 10, (0, 3), aoe: 1, vertical: 4, master_rp: 110,
        effect: fx(Status::Regen, 100, 4)),
    // Lama d'ombra
    ab!(Backstab, "backstab", "Pugnalata", Physical, 125, 4, (1, 1), effect: fx(Status::Poison, 25, 3)),
    ab!(SleepBlade, "sleep_blade", "Lama soporifera", Physical, 80, 8, (1, 1), master_rp: 110,
        effect: fx(Status::Sleep, 45, 2)),
    ab!(VenomFang, "venom_fang", "Zanna velenosa", Physical, 95, 6, (1, 1), master_rp: 90,
        effect: fx(Status::Poison, 65, 4)),
    ab!(BladeDance, "blade_dance", "Danza delle lame", Physical, 85, 10, (1, 1), aoe: 1, master_rp: 130),
    ab!(ShadowVeil, "shadow_veil", "Velo d'ombra", Support, 0, 8, (0, 0), vertical: 4, master_rp: 100,
        effect: fx(Status::Haste, 100, 3)),
    // Dragone
    ab!(Leap, "leap", "Salto", Physical, 135, 6, (2, 3), vertical: 9),
    ab!(DiveStrike, "dive_strike", "Picchiata", Physical, 190, 14, (2, 4), vertical: 10, master_rp: 150),
    ab!(DragonLance, "dragon_lance", "Lancia del drago", Physical, 120, 8, (1, 2), vertical: 4,
        element: Some(Fire), master_rp: 110),
    ab!(Roar, "roar", "Ruggito", Support, 0, 8, (0, 0), aoe: 1, vertical: 4, master_rp: 100,
        effect: fx(Status::Slow, 70, 3)),
    // Bardo
    ab!(Ballad, "ballad", "Ballata", Support, 0, 6, (0, 0), aoe: 2, vertical: 4, effect: fx(Status::Regen, 100, 3)),
    ab!(WarSong, "war_song", "Canto di guerra", Support, 0, 12, (0, 0), aoe: 2, vertical: 4, master_rp: 130,
        effect: fx(Status::Haste, 100, 3)),
    ab!(Lullaby, "lullaby", "Ninna nanna", Support, 0, 12, (1, 4), aoe: 1, vertical: 4, master_rp: 120,
        effect: fx(Status::Sleep, 55, 2)),
    ab!(Dirge, "dirge", "Nenia", Support, 0, 8, (1, 4), aoe: 1, vertical: 4, master_rp: 100,
        effect: fx(Status::Slow, 60, 3)),
    ab!(Hymn, "hymn", "Inno", Heal, 70, 14, (0, 0), aoe: 2, vertical: 4, master_rp: 120),
    // Alchimista
    ab!(Lob, "lob", "Lancio", Physical, 105, 0, (2, 4), vertical: 5, ranged: true),
    ab!(Blastmix, "blastmix", "Miscela esplosiva", Magic, 125, 9, (2, 4), aoe: 1, vertical: 5,
        element: Some(Fire), ranged: true, master_rp: 110),
    ab!(ToxicFumes, "toxic_fumes", "Vapori tossici", Support, 0, 8, (1, 4), aoe: 1, vertical: 4,
        ranged: true, master_rp: 100, effect: fx(Status::Poison, 70, 4)),
    ab!(Panacea, "panacea", "Panacea", Support, 0, 6, (0, 2), aoe: 1, vertical: 4, master_rp: 90, cleanse: true),
    ab!(Tonic, "tonic", "Tonico", Heal, 130, 6, (0, 1), vertical: 4, master_rp: 90),
    // Geomante
    ab!(Landslide, "landslide", "Frana", Magic, 130, 5, (1, 3), vertical: 6, effect: fx(Status::Root, 20, 2)),
    ab!(Quicksand, "quicksand", "Sabbie mobili", Support, 0, 10, (1, 4), aoe: 1, vertical: 4, master_rp: 110,
        effect: fx(Status::Root, 70, 2)),
    ab!(Earthquake, "earthquake", "Terremoto", Magic, 120, 20, (2, 4), aoe: 2, vertical: 6, master_rp: 160),
    ab!(Stoneskin, "stoneskin", "Pelle di pietra", Support, 0, 8, (0, 2), aoe: 1, vertical: 4, master_rp: 100,
        effect: fx(Status::Aegis, 100, 3)),
    ab!(Sap, "sap", "Linfa", Support, 0, 8, (0, 3), aoe: 1, vertical: 4, master_rp: 90,
        effect: fx(Status::Regen, 100, 3)),
    // Invocatore
    ab!(Spark, "spark", "Scintilla", Magic, 100, 4, (1, 4), vertical: 5, element: Some(Holy)),
    ab!(FireSpirit, "fire_spirit", "Spirito del fuoco", Magic, 150, 30, (2, 5), aoe: 2, vertical: 6,
        element: Some(Fire), master_rp: 180),
    ab!(FrostSpirit, "frost_spirit", "Spirito del gelo", Magic, 140, 28, (2, 5), aoe: 2, vertical: 6,
        element: Some(Ice), master_rp: 170, effect: fx(Status::Slow, 40, 2)),
    ab!(ThunderSpirit, "thunder_spirit", "Spirito del tuono", Magic, 165, 26, (2, 5), aoe: 1, vertical: 7,
        element: Some(Thunder), master_rp: 170, effect: fx(Status::Root, 35, 2)),
    ab!(DawnSpirit, "dawn_spirit", "Spirito dell'alba", Heal, 110, 28, (0, 4), aoe: 2, vertical: 6,
        master_rp: 170),
    // Consumabili: valori fissi, portata corta (le bombe si lanciano).
    ab!(Potion, "potion", "Pozione", Heal, 50, 0, (0, 1), vertical: 3, fixed: true),
    ab!(Ether, "ether", "Etere", Heal, 25, 0, (0, 1), vertical: 3, fixed: true, restore_mp: true),
    ab!(Remedy, "remedy", "Rimedio", Support, 0, 0, (0, 1), vertical: 3, cleanse: true),
    ab!(Revive, "revive", "Essenza vitale", Heal, 30, 0, (1, 1), vertical: 3, revive: true),
    ab!(Elixir, "elixir", "Elisir", Heal, 999, 0, (0, 1), vertical: 3, fixed: true),
    ab!(FireBomb, "fire_bomb", "Bomba di brace", Magic, 45, 0, (1, 4), aoe: 1, vertical: 4,
        element: Some(Fire), ranged: true, fixed: true),
    ab!(FrostVial, "frost_vial", "Fiala di gelo", Magic, 35, 0, (1, 4), vertical: 4,
        element: Some(Ice), ranged: true, fixed: true, effect: fx(Status::Slow, 70, 3)),
    ab!(ToxinVial, "toxin_vial", "Fiala tossica", Support, 0, 0, (1, 4), aoe: 1, vertical: 4,
        ranged: true, effect: fx(Status::Poison, 85, 4)),
    ab!(SleepPowder, "sleep_powder", "Polvere del sonno", Support, 0, 0, (1, 3), aoe: 1, vertical: 4,
        ranged: true, effect: fx(Status::Sleep, 55, 2)),
];

impl AbilityId {
    pub const ALL: [AbilityId; 84] = {
        let mut out = [AbilityId::Strike; 84];
        let mut i = 0;
        while i < 84 {
            out[i] = TABLE[i].id;
            i += 1;
        }
        out
    };

    pub fn def(self) -> &'static Ability {
        &TABLE[self as usize]
    }

    pub fn key(self) -> &'static str {
        self.def().key
    }

    pub fn from_key(key: &str) -> Option<AbilityId> {
        Self::ALL.into_iter().find(|a| a.key() == key)
    }

    /// Classe che insegna l'abilità (`None` per quelle universali).
    pub fn school(self) -> Option<Class> {
        Class::ALL
            .into_iter()
            .find(|c| c.innate().contains(&self) || c.learnable().contains(&self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_matches_enum() {
        for (i, a) in TABLE.iter().enumerate() {
            assert_eq!(a.id as usize, i, "{}", a.name);
            assert_eq!(AbilityId::from_key(a.key), Some(a.id));
        }
    }
}

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
static TABLE: [Ability; 36] = [
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
    ab!(Flame, "flame", "Fiamma", Magic, 110, 8, (0, 4), aoe: 1, vertical: 4, element: Some(Fire)),
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
    ab!(Rebuke, "rebuke", "Ammonimento", Magic, 80, 4, (1, 3), vertical: 4, element: Some(Holy),
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
    pub const ALL: [AbilityId; 36] = {
        let mut out = [AbilityId::Strike; 36];
        let mut i = 0;
        while i < 36 {
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

//! Tabella delle abilità.
//!
//! Ogni classe ha abilità innate e abilità della propria "scuola" che si usano
//! solo finché un oggetto equipaggiato le insegna, oppure per sempre una volta
//! padroneggiate accumulando punti risonanza (vedi `progression`).

use crate::class::Class;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Physical,
    Magic,
    Heal,
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
};

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
static TABLE: [Ability; 20] = [
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
    ab!(Frost, "frost", "Gelo", Magic, 110, 8, (0, 4), aoe: 1, vertical: 4, element: Some(Ice), master_rp: 80),
    ab!(Bolt, "bolt", "Folgore", Magic, 160, 14, (1, 4), vertical: 6, element: Some(Thunder), master_rp: 100),
    ab!(Inferno, "inferno", "Vampa", Magic, 150, 18, (2, 4), aoe: 1, vertical: 4, element: Some(Fire), master_rp: 150),
    ab!(Mend, "mend", "Cura", Heal, 100, 6, (0, 3), aoe: 1, vertical: 4),
    ab!(Radiance, "radiance", "Luce", Magic, 90, 6, (1, 3), vertical: 4, element: Some(Holy), master_rp: 80),
    ab!(GreatMend, "great_mend", "Cura maggiore", Heal, 140, 14, (0, 3), aoe: 1, vertical: 4, master_rp: 120),
    ab!(RuneEdge, "rune_edge", "Lama runica", Physical, 120, 4, (1, 1), element: Some(Thunder)),
    ab!(DrainBlade, "drain_blade", "Lama vorace", Physical, 110, 6, (1, 1), drain: true, master_rp: 100),
    ab!(FrostEdge, "frost_edge", "Taglio gelido", Physical, 130, 8, (1, 1), element: Some(Ice), master_rp: 100),
    ab!(Rebuke, "rebuke", "Ammonimento", Magic, 80, 4, (1, 3), vertical: 4, element: Some(Holy)),
    ab!(Sanctuary, "sanctuary", "Santuario", Heal, 60, 12, (0, 2), aoe: 2, vertical: 4, master_rp: 120),
    ab!(Judgment, "judgment", "Giudizio", Magic, 140, 16, (1, 4), vertical: 6, element: Some(Holy), master_rp: 140),
];

impl AbilityId {
    pub const ALL: [AbilityId; 20] = {
        let mut out = [AbilityId::Strike; 20];
        let mut i = 0;
        while i < 20 {
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

//! Status alterati. Le durate si contano nei turni dell'unità che li subisce
//! (diminuiscono alla fine di ogni suo turno, anche se lo salta dormendo).

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Status {
    /// Perde l'8% degli HP massimi a fine turno.
    Poison,
    /// Salta i turni; si sveglia se subisce danni. I colpi fisici vanno sempre a segno.
    Sleep,
    /// Non può usare magie, cure e incantesimi di supporto.
    Silence,
    /// Accumula iniziativa a metà velocità.
    Slow,
    /// Non può muoversi.
    Root,
    /// Accumula iniziativa una volta e mezza più in fretta.
    Haste,
    /// Riduce di un terzo i danni fisici subiti.
    Aegis,
    /// Recupera l'8% degli HP massimi a fine turno.
    Regen,
}

/// Effetto di un'abilità: status, probabilità (%) e durata (turni).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StatusEffect {
    pub status: Status,
    pub chance: i32,
    pub duration: i32,
}

pub const TICK_PCT: i32 = 8;

impl Status {
    pub const ALL: [Status; 8] = [
        Status::Poison,
        Status::Sleep,
        Status::Silence,
        Status::Slow,
        Status::Root,
        Status::Haste,
        Status::Aegis,
        Status::Regen,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Status::Poison => "Veleno",
            Status::Sleep => "Sonno",
            Status::Silence => "Silenzio",
            Status::Slow => "Lentezza",
            Status::Root => "Radici",
            Status::Haste => "Rapidità",
            Status::Aegis => "Egida",
            Status::Regen => "Rigenerazione",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Status::Poison => "poison",
            Status::Sleep => "sleep",
            Status::Silence => "silence",
            Status::Slow => "slow",
            Status::Root => "root",
            Status::Haste => "haste",
            Status::Aegis => "aegis",
            Status::Regen => "regen",
        }
    }

    pub fn is_negative(self) -> bool {
        matches!(
            self,
            Status::Poison | Status::Sleep | Status::Silence | Status::Slow | Status::Root
        )
    }

    /// Status che si annullano a vicenda.
    pub fn opposite(self) -> Option<Status> {
        match self {
            Status::Haste => Some(Status::Slow),
            Status::Slow => Some(Status::Haste),
            _ => None,
        }
    }
}

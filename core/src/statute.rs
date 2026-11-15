//! Statuti: regole estratte a ogni battaglia. Violarli comporta una sanzione
//! (sospensione per alcuni turni, espulsione alla seconda).

use crate::ability::{Ability, AbilityId, Element, Kind};
use crate::rng::Rng;

pub const SUSPEND_TURNS: i32 = 2;
pub const EXPEL_AT: i32 = 2;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Statute {
    NoHealing,
    NoElement(Element),
    NoRanged,
    NoMagic,
    /// Vietato ripetere l'ultima abilità usata da chiunque.
    NoEcho,
    /// Vietato colpire bersagli più in basso di sé.
    NoDownwardStrike,
}

pub struct StatuteCtx {
    pub user_height: i32,
    pub target_height: i32,
    pub previous_ability: Option<AbilityId>,
}

impl Statute {
    pub const POOL: [Statute; 8] = [
        Statute::NoHealing,
        Statute::NoElement(Element::Fire),
        Statute::NoElement(Element::Ice),
        Statute::NoElement(Element::Thunder),
        Statute::NoRanged,
        Statute::NoMagic,
        Statute::NoEcho,
        Statute::NoDownwardStrike,
    ];

    pub fn name(self) -> String {
        match self {
            Statute::NoHealing => "Niente cure".into(),
            Statute::NoElement(e) => format!("Niente {}", e.name()),
            Statute::NoRanged => "Niente armi a distanza".into(),
            Statute::NoMagic => "Niente magia offensiva".into(),
            Statute::NoEcho => "Divieto d'eco".into(),
            Statute::NoDownwardStrike => "Divieto di colpire dall'alto".into(),
        }
    }

    pub fn is_violated(self, ab: &Ability, ctx: &StatuteCtx) -> bool {
        match self {
            Statute::NoHealing => ab.kind == Kind::Heal,
            Statute::NoElement(e) => ab.element == Some(e),
            Statute::NoRanged => ab.ranged,
            Statute::NoMagic => ab.kind == Kind::Magic,
            Statute::NoEcho => ctx.previous_ability == Some(ab.id),
            Statute::NoDownwardStrike => {
                ab.kind != Kind::Heal && ctx.target_height < ctx.user_height
            }
        }
    }

    /// Estrae `n` statuti distinti.
    pub fn draw(rng: &mut Rng, n: usize) -> Vec<Statute> {
        let mut pool = Self::POOL.to_vec();
        rng.shuffle(&mut pool);
        pool.truncate(n.min(pool.len()));
        pool
    }
}

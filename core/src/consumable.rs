//! Consumabili: pergamene (un incantesimo, una volta) e pozioni, bombe e fiale.
//!
//! Si portano nella borsa di battaglia (al massimo `BAG_SLOTS` per unità) e
//! usarne uno è l'azione del turno. La qualità (1-6, in numeri romani nel nome)
//! dipende dal livello di chi li lascia: aumenta la potenza minima delle
//! pergamene e il valore di pozioni e bombe.

use crate::ability::{AbilityId, Kind};
use crate::gear::{Rarity, TIERS};

/// Consumabili che un'unità può portare in battaglia.
pub const BAG_SLOTS: usize = 2;

pub struct ConsumableDef {
    pub code: &'static str,
    pub name: &'static str,
    pub ability: AbilityId,
    /// Pergamena: lancia un incantesimo (bloccata dal Silenzio).
    pub scroll: bool,
    pub rarity: Rarity,
    pub price: i32,
    pub description: &'static str,
}

macro_rules! cons {
    ($code:literal, $name:literal, $ab:ident, $scroll:literal, $rar:ident, $price:literal, $desc:literal) => {
        ConsumableDef {
            code: $code,
            name: $name,
            ability: AbilityId::$ab,
            scroll: $scroll,
            rarity: Rarity::$rar,
            price: $price,
            description: $desc,
        }
    };
}

pub static CONSUMABLES: [ConsumableDef; 32] = [
    // Pozioni e preparati
    cons!(
        "pozione",
        "Pozione",
        Potion,
        false,
        Common,
        40,
        "Cura gli HP di sé o di un alleato vicino."
    ),
    cons!(
        "etere",
        "Etere",
        Ether,
        false,
        Uncommon,
        70,
        "Ripristina gli MP di sé o di un alleato vicino."
    ),
    cons!(
        "rimedio",
        "Rimedio",
        Remedy,
        false,
        Common,
        50,
        "Toglie gli status negativi."
    ),
    cons!(
        "essenza",
        "Essenza vitale",
        Revive,
        false,
        Rare,
        220,
        "Rianima un alleato sconfitto accanto, se la sua casella è libera."
    ),
    cons!(
        "elisir",
        "Elisir",
        Elixir,
        false,
        Epic,
        400,
        "Riporta gli HP al massimo."
    ),
    cons!(
        "bomba",
        "Bomba di brace",
        FireBomb,
        false,
        Uncommon,
        90,
        "Si lancia: fuoco su un'area."
    ),
    cons!(
        "gelo",
        "Fiala di gelo",
        FrostVial,
        false,
        Uncommon,
        90,
        "Si lancia: ghiaccio e Lentezza."
    ),
    cons!(
        "tossina",
        "Fiala tossica",
        ToxinVial,
        false,
        Uncommon,
        80,
        "Si lancia: Veleno su un'area."
    ),
    cons!(
        "sonno",
        "Polvere del sonno",
        SleepPowder,
        false,
        Rare,
        140,
        "Si lancia: Sonno su un'area."
    ),
    // Pergamene: un incantesimo, una volta, per qualsiasi classe
    cons!(
        "p_fiamma",
        "Pergamena di Fiamma",
        Flame,
        true,
        Uncommon,
        90,
        ""
    ),
    cons!(
        "p_gelo",
        "Pergamena di Gelo",
        Frost,
        true,
        Uncommon,
        100,
        ""
    ),
    cons!(
        "p_folgore",
        "Pergamena di Folgore",
        Bolt,
        true,
        Rare,
        180,
        ""
    ),
    cons!(
        "p_vampa",
        "Pergamena di Vampa",
        Inferno,
        true,
        Epic,
        260,
        ""
    ),
    cons!(
        "p_luce",
        "Pergamena di Luce",
        Radiance,
        true,
        Uncommon,
        90,
        ""
    ),
    cons!(
        "p_giudizio",
        "Pergamena del Giudizio",
        Judgment,
        true,
        Epic,
        260,
        ""
    ),
    cons!(
        "p_catene",
        "Pergamena delle Catene",
        Chains,
        true,
        Rare,
        150,
        ""
    ),
    cons!(
        "p_torpore",
        "Pergamena del Torpore",
        Torpor,
        true,
        Rare,
        160,
        ""
    ),
    cons!("p_cura", "Pergamena di Cura", Mend, true, Common, 70, ""),
    cons!(
        "p_cura_m",
        "Pergamena di Cura maggiore",
        GreatMend,
        true,
        Rare,
        170,
        ""
    ),
    cons!(
        "p_santuario",
        "Pergamena del Santuario",
        Sanctuary,
        true,
        Rare,
        180,
        ""
    ),
    cons!(
        "p_egida",
        "Pergamena dell'Egida",
        Aegis,
        true,
        Uncommon,
        90,
        ""
    ),
    cons!(
        "p_germoglio",
        "Pergamena del Germoglio",
        Sprout,
        true,
        Uncommon,
        100,
        ""
    ),
    cons!(
        "p_purif",
        "Pergamena di Purificazione",
        Purify,
        true,
        Uncommon,
        80,
        ""
    ),
    cons!(
        "p_slancio",
        "Pergamena dello Slancio",
        Rally,
        true,
        Rare,
        140,
        ""
    ),
    cons!(
        "p_bufera",
        "Pergamena della Bufera",
        Blizzard,
        true,
        Epic,
        280,
        ""
    ),
    cons!(
        "p_temporale",
        "Pergamena del Temporale",
        Thunderstorm,
        true,
        Rare,
        200,
        ""
    ),
    cons!(
        "p_meteora",
        "Pergamena della Meteora",
        Meteor,
        true,
        Epic,
        320,
        ""
    ),
    cons!(
        "p_silenzio",
        "Pergamena del Silenzio arcano",
        ArcaneHush,
        true,
        Rare,
        150,
        ""
    ),
    cons!(
        "p_preghiera",
        "Pergamena della Preghiera",
        Prayer,
        true,
        Rare,
        190,
        ""
    ),
    cons!(
        "p_raggio",
        "Pergamena del Raggio sacro",
        HolyRay,
        true,
        Uncommon,
        110,
        ""
    ),
    cons!(
        "p_ritorno",
        "Pergamena del Ritorno",
        Return,
        true,
        Epic,
        300,
        ""
    ),
    cons!("p_voto", "Pergamena del Voto", Vow, true, Uncommon, 100, ""),
];

const ROMAN: [&str; TIERS] = ["I", "II", "III", "IV", "V", "VI"];
/// Valore dei consumabili a valore fisso per qualità (in centesimi).
const TIER_PCT: [i32; TIERS] = [100, 150, 210, 280, 360, 450];

/// Un consumabile: tipo e qualità.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Consumable {
    pub kind: u8,
    pub tier: u8,
}

/// Come si lancia l'abilità di un consumabile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cast {
    pub ability: AbilityId,
    /// MAG minima di chi la lancia (pergamene).
    pub min_mag: i32,
    /// Moltiplicatore dei valori fissi (in centesimi).
    pub power_pct: i32,
    /// Bloccata dal Silenzio.
    pub silenceable: bool,
}

impl Consumable {
    pub fn def(&self) -> &'static ConsumableDef {
        &CONSUMABLES[self.kind as usize]
    }

    pub fn key(&self) -> String {
        format!("c.{}.{}", self.def().code, self.tier)
    }

    pub fn from_key(key: &str) -> Option<Consumable> {
        let p: Vec<&str> = key.split('.').collect();
        let ["c", code, tier] = p.as_slice() else {
            return None;
        };
        let kind = CONSUMABLES.iter().position(|c| c.code == *code)? as u8;
        let tier: u8 = tier.parse().ok().filter(|&t: &u8| (t as usize) < TIERS)?;
        Some(Consumable { kind, tier })
    }

    pub fn name(&self) -> String {
        format!("{} {}", self.def().name, ROMAN[self.tier as usize])
    }

    pub fn rarity(&self) -> Rarity {
        self.def().rarity
    }

    pub fn price(&self) -> i32 {
        self.def().price * TIER_PCT[self.tier as usize] / 100
    }

    pub fn cast(&self) -> Cast {
        let d = self.def();
        Cast {
            ability: d.ability,
            min_mag: if d.scroll {
                18 + 7 * self.tier as i32
            } else {
                0
            },
            power_pct: TIER_PCT[self.tier as usize],
            silenceable: d.scroll,
        }
    }

    /// Descrizione per l'interfaccia, con i valori della qualità attuale.
    pub fn describe(&self) -> String {
        let d = self.def();
        let ab = d.ability.def();
        if d.scroll {
            let what = match ab.kind {
                Kind::Magic => "incantesimo d'attacco",
                Kind::Heal => "cura",
                _ => "incantesimo di supporto",
            };
            return format!(
                "Lancia {} ({what}) una volta, senza MP, con MAG almeno {}. Qualsiasi classe; non sotto Silenzio.",
                ab.name,
                self.cast().min_mag
            );
        }
        let amount = ab.power * TIER_PCT[self.tier as usize] / 100;
        let value = match d.ability {
            AbilityId::Potion => format!(" (+{amount} HP)"),
            AbilityId::Ether => format!(" (+{amount} MP)"),
            AbilityId::FireBomb | AbilityId::FrostVial => format!(" ({amount} danni)"),
            AbilityId::Revive => format!(" (con il {}% degli HP)", ab.power),
            _ => String::new(),
        };
        format!("{}{value}", d.description)
    }
}

/// Cast di un'abilità normale (nessun minimo, nessun moltiplicatore).
impl From<AbilityId> for Cast {
    fn from(ability: AbilityId) -> Self {
        Cast {
            ability,
            min_mag: 0,
            power_pct: 100,
            silenceable: true,
        }
    }
}

/// Consumabile casuale di questa rarità (o il più vicino per difetto).
pub fn roll(rng: &mut crate::rng::Rng, rarity: Rarity, tier: u8) -> Consumable {
    let mut r = rarity.min(Rarity::Epic);
    loop {
        let pool: Vec<u8> = (0..CONSUMABLES.len() as u8)
            .filter(|&i| CONSUMABLES[i as usize].rarity == r)
            .collect();
        if !pool.is_empty() {
            let kind = pool[rng.below(pool.len() as u32) as usize];
            // Un leggendario diventa un epico di qualità superiore.
            let bump = if rarity == Rarity::Legendary { 1 } else { 0 };
            return Consumable {
                kind,
                tier: (tier + bump).min(TIERS as u8 - 1),
            };
        }
        r = match r {
            Rarity::Epic => Rarity::Rare,
            Rarity::Rare => Rarity::Uncommon,
            _ => Rarity::Common,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_roundtrip() {
        for kind in 0..CONSUMABLES.len() as u8 {
            for tier in 0..TIERS as u8 {
                let c = Consumable { kind, tier };
                assert_eq!(Consumable::from_key(&c.key()), Some(c));
            }
        }
        assert_eq!(Consumable { kind: 0, tier: 2 }.name(), "Pozione III");
    }

    #[test]
    fn scrolls_are_school_spells_and_items_are_not() {
        for c in &CONSUMABLES {
            if c.scroll {
                assert!(c.ability.school().is_some(), "{}", c.name);
            } else {
                assert!(c.ability.school().is_none(), "{}", c.name);
            }
        }
    }
}

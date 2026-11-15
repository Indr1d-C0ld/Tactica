//! Mestieri: materiali, smantellamento, forgia e alchimia.
//!
//! - **Smantellare** un oggetto restituisce materiali della sua famiglia
//!   (metallo, legno, stoffa, cuoio, gemme, carta) e polvere arcana se è raro o più.
//! - **Forgia:** *raffinare* alza di uno la qualità del materiale; *incantare*
//!   alza di uno la rarità aggiungendo un affisso (fino a epico).
//! - **Alchimia:** ricette che trasformano erbe, polvere arcana e altri
//!   materiali in consumabili.
//!
//! Tutto è deterministico dato il generatore: il server lo inizializza a caso.

use crate::consumable::{CONSUMABLES, Consumable};
use crate::gear::{AFFIXES, Family, Form, Gear, ItemRef, Rarity, TIERS};
use crate::rng::Rng;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Material {
    Metal,
    Wood,
    Cloth,
    Leather,
    Gems,
    Paper,
    ArcaneDust,
    Herbs,
}

impl Material {
    pub const ALL: [Material; 8] = [
        Material::Metal,
        Material::Wood,
        Material::Cloth,
        Material::Leather,
        Material::Gems,
        Material::Paper,
        Material::ArcaneDust,
        Material::Herbs,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Material::Metal => "metal",
            Material::Wood => "wood",
            Material::Cloth => "cloth",
            Material::Leather => "leather",
            Material::Gems => "gems",
            Material::Paper => "paper",
            Material::ArcaneDust => "arcane_dust",
            Material::Herbs => "herbs",
        }
    }

    pub fn from_key(k: &str) -> Option<Material> {
        Material::ALL.into_iter().find(|m| m.key() == k)
    }

    pub fn name(self) -> &'static str {
        match self {
            Material::Metal => "Rottami di metallo",
            Material::Wood => "Legno lavorato",
            Material::Cloth => "Ritagli di stoffa",
            Material::Leather => "Strisce di cuoio",
            Material::Gems => "Schegge di gemma",
            Material::Paper => "Fogli di pergamena",
            Material::ArcaneDust => "Polvere arcana",
            Material::Herbs => "Erbe officinali",
        }
    }

    fn of_family(f: Family) -> Material {
        match f {
            Family::Metal => Material::Metal,
            Family::Wood => Material::Wood,
            Family::Cloth => Material::Cloth,
            Family::Leather => Material::Leather,
            Family::Jewel => Material::Gems,
            Family::Paper => Material::Paper,
        }
    }
}

/// Quantità di materiali (lista ordinata, senza zeri).
pub type Bundle = Vec<(Material, i32)>;

fn add(b: &mut Bundle, m: Material, n: i32) {
    if n <= 0 {
        return;
    }
    match b.iter_mut().find(|(x, _)| *x == m) {
        Some(e) => e.1 += n,
        None => b.push((m, n)),
    }
    b.sort();
}

/// Materiali ottenuti smantellando un oggetto.
pub fn salvage(item: ItemRef) -> Bundle {
    let mut b = Bundle::new();
    let rarity = item.rarity() as i32;
    match item {
        ItemRef::Gen(g) => {
            add(
                &mut b,
                Material::of_family(g.base().family),
                1 + g.tier as i32,
            );
            add(&mut b, Material::ArcaneDust, rarity - 1);
        }
        ItemRef::Fixed(f) => {
            let fam = crate::gear::BASES
                .iter()
                .find(|b| b.code == item.icon())
                .map_or(Family::Metal, |b| b.family);
            add(&mut b, Material::of_family(fam), 1 + f.def().price / 200);
            add(&mut b, Material::ArcaneDust, rarity - 1);
        }
        ItemRef::Use(c) => {
            let scroll = c.def().scroll;
            add(
                &mut b,
                if scroll {
                    Material::Paper
                } else {
                    Material::Herbs
                },
                1,
            );
            add(
                &mut b,
                Material::ArcaneDust,
                (rarity - 1).max(0) + c.tier as i32 / 2,
            );
        }
    }
    b
}

/// Costo di un'operazione: materiali e monete.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cost {
    pub materials: Bundle,
    pub coins: i32,
}

/// Raffinare: qualità del materiale +1 (solo oggetti generati, non leggendari al massimo).
pub fn refine_cost(item: ItemRef) -> Option<Cost> {
    let ItemRef::Gen(g) = item else { return None };
    if g.tier as usize + 1 >= TIERS {
        return None;
    }
    let t = g.tier as i32 + 1;
    let mut materials = Bundle::new();
    add(&mut materials, Material::of_family(g.base().family), 3 * t);
    add(&mut materials, Material::ArcaneDust, t / 2);
    Some(Cost {
        materials,
        coins: 60 * t,
    })
}

pub fn refine(item: ItemRef) -> Option<ItemRef> {
    refine_cost(item)?;
    let ItemRef::Gen(mut g) = item else {
        return None;
    };
    g.tier += 1;
    Some(ItemRef::Gen(g))
}

/// Incantare: rarità +1 con un affisso in più (da comune fino a epico).
pub fn enchant_cost(item: ItemRef) -> Option<Cost> {
    let ItemRef::Gen(g) = item else { return None };
    if g.legend().is_some() || g.rarity >= Rarity::Epic {
        return None;
    }
    let r = g.rarity as i32 + 1;
    let mut materials = Bundle::new();
    add(&mut materials, Material::ArcaneDust, 2 * r);
    add(&mut materials, Material::Gems, r);
    Some(Cost {
        materials,
        coins: 120 * r,
    })
}

pub fn enchant(item: ItemRef, rng: &mut Rng) -> Option<ItemRef> {
    enchant_cost(item)?;
    let ItemRef::Gen(g) = item else { return None };
    let rarity = match g.rarity {
        Rarity::Common => Rarity::Uncommon,
        Rarity::Uncommon => Rarity::Rare,
        _ => Rarity::Epic,
    };
    Some(ItemRef::Gen(add_affix(g, rarity, rng)))
}

/// Aggiunge un affisso compatibile (stesse regole della generazione).
fn add_affix(mut g: Gear, rarity: Rarity, rng: &mut Rng) -> Gear {
    g.rarity = rarity;
    let slot = g.base().slot;
    let used: Vec<u8> = g
        .affixes
        .iter()
        .copied()
        .filter(|&a| a != u8::MAX)
        .collect();
    let Some(free) = g.affixes.iter().position(|&a| a == u8::MAX) else {
        return g;
    };
    let has_prefix = used
        .iter()
        .any(|&a| matches!(AFFIXES[a as usize].form, Form::Prefix(..)));
    let candidates: Vec<u8> = (0..AFFIXES.len() as u8)
        .filter(|a| !used.contains(a))
        .filter(|&a| {
            let d = &AFFIXES[a as usize];
            d.slots.contains(&slot)
                && d.min <= rarity
                && !(has_prefix && matches!(d.form, Form::Prefix(..)))
                && !used.iter().any(|&o| {
                    let od = &AFFIXES[o as usize];
                    matches!((d.effect, od.effect), (Some(x), Some(y))
                        if std::mem::discriminant(&x) == std::mem::discriminant(&y))
                })
        })
        .collect();
    if candidates.is_empty() {
        return g;
    }
    g.affixes[free] = candidates[rng.below(candidates.len() as u32) as usize];
    // Prima gli aggettivi, poi i complementi.
    let n = g.affixes.iter().filter(|&&a| a != u8::MAX).count();
    g.affixes[..n].sort_by_key(|&a| (matches!(AFFIXES[a as usize].form, Form::Suffix(_)), a));
    g
}

/// Ricetta di alchimia: un consumabile di una certa qualità.
pub struct Recipe {
    pub code: &'static str,
    /// Codice del consumabile prodotto.
    pub makes: &'static str,
    pub materials: &'static [(Material, i32)],
    pub coins: i32,
}

macro_rules! recipe {
    ($code:literal, $makes:literal, [$(($m:ident, $n:literal)),*], $coins:literal) => {
        Recipe { code: $code, makes: $makes, materials: &[$((Material::$m, $n)),*], coins: $coins }
    };
}

pub static RECIPES: [Recipe; 16] = [
    recipe!("pozione", "pozione", [(Herbs, 2)], 10),
    recipe!("etere", "etere", [(Herbs, 2), (ArcaneDust, 1)], 20),
    recipe!("rimedio", "rimedio", [(Herbs, 3)], 15),
    recipe!(
        "essenza",
        "essenza",
        [(Herbs, 3), (ArcaneDust, 3), (Gems, 1)],
        80
    ),
    recipe!(
        "elisir",
        "elisir",
        [(Herbs, 5), (ArcaneDust, 5), (Gems, 2)],
        150
    ),
    recipe!("bomba", "bomba", [(Metal, 2), (ArcaneDust, 1)], 25),
    recipe!("gelo", "gelo", [(Herbs, 1), (Gems, 1), (ArcaneDust, 1)], 25),
    recipe!("tossina", "tossina", [(Herbs, 3), (Leather, 1)], 20),
    recipe!(
        "sonno",
        "sonno",
        [(Herbs, 2), (Cloth, 2), (ArcaneDust, 1)],
        40
    ),
    recipe!("p_fiamma", "p_fiamma", [(Paper, 2), (ArcaneDust, 1)], 30),
    recipe!("p_gelo", "p_gelo", [(Paper, 2), (ArcaneDust, 1)], 30),
    recipe!("p_cura", "p_cura", [(Paper, 2), (Herbs, 1)], 25),
    recipe!("p_folgore", "p_folgore", [(Paper, 3), (ArcaneDust, 2)], 60),
    recipe!(
        "p_egida",
        "p_egida",
        [(Paper, 2), (Metal, 1), (ArcaneDust, 1)],
        35
    ),
    recipe!(
        "p_ritorno",
        "p_ritorno",
        [(Paper, 4), (ArcaneDust, 4), (Herbs, 3)],
        160
    ),
    recipe!(
        "p_meteora",
        "p_meteora",
        [(Paper, 5), (ArcaneDust, 6), (Gems, 2)],
        220
    ),
];

/// Qualità massima producibile (I-III): le più alte si trovano solo come bottino.
pub const MAX_BREW_TIER: u8 = 2;

impl Recipe {
    pub fn find(code: &str) -> Option<&'static Recipe> {
        RECIPES.iter().find(|r| r.code == code)
    }

    /// Costo per la qualità `tier` (0 = I): materiali e monete crescono con la qualità.
    pub fn cost(&self, tier: u8) -> Cost {
        let k = tier as i32 + 1;
        let mut materials = Bundle::new();
        for &(m, n) in self.materials {
            add(&mut materials, m, n * k);
        }
        Cost {
            materials,
            coins: self.coins * k,
        }
    }

    pub fn product(&self, tier: u8) -> Option<ItemRef> {
        let kind = CONSUMABLES.iter().position(|c| c.code == self.makes)? as u8;
        Some(ItemRef::Use(Consumable {
            kind,
            tier: tier.min(MAX_BREW_TIER),
        }))
    }
}

/// Erbe a fine partita: 1, più 1 se si vince.
pub fn herbs_reward(win: bool) -> i32 {
    1 + win as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gear::{ItemRef, roll_item};

    #[test]
    fn every_recipe_makes_a_real_consumable() {
        for r in &RECIPES {
            for t in 0..=MAX_BREW_TIER {
                assert!(r.product(t).is_some(), "{}", r.code);
            }
            assert!(!r.cost(0).materials.is_empty());
        }
    }

    #[test]
    fn salvage_refine_and_enchant() {
        let mut rng = Rng::new(9);
        let mut refined = 0;
        let mut enchanted = 0;
        for i in 0..2000 {
            let item = roll_item(&mut rng, 1 + i % 30, 0);
            assert!(!salvage(item).is_empty(), "{}", item.key());
            if let Some(up) = refine(item) {
                let (ItemRef::Gen(a), ItemRef::Gen(b)) = (item, up) else {
                    panic!()
                };
                assert_eq!(b.tier, a.tier + 1);
                assert!(up.bonus().atk >= item.bonus().atk);
                refined += 1;
            }
            if let Some(up) = enchant(item, &mut rng) {
                assert_eq!(up.rarity() as i32, item.rarity() as i32 + 1);
                // La nuova chiave è valida e il nome resta leggibile.
                assert_eq!(ItemRef::from_key(&up.key()), Some(up));
                enchanted += 1;
            }
        }
        assert!(refined > 500 && enchanted > 500);
    }
}

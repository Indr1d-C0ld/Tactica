//! Progressione tra una battaglia e l'altra.
//!
//! **Risonanza:** ogni oggetto equipaggiato che insegna un'abilità della scuola
//! della classe attuale fa accumulare punti risonanza (RP) a quell'abilità.
//! Raggiunta la soglia `master_rp`, l'abilità è padroneggiata per sempre e
//! resta usabile anche senza l'oggetto (quando l'unità è in quella classe).
//! Le abilità padroneggiate sbloccano le classi avanzate.

use crate::ability::AbilityId;
use crate::class::Class;
use crate::item::ItemId;
use crate::rng::Rng;
use crate::unit::Unit;

pub const RP_BASE: i32 = 20;
pub const RP_WIN: i32 = 10;
pub const RP_PER_ACTION: i32 = 2;
pub const RP_ACTION_CAP: i32 = 10;

pub const COINS_WIN: i64 = 120;
pub const COINS_DRAW: i64 = 70;
pub const COINS_LOSS: i64 = 40;
/// Probabilità (%) di bottino in caso di vittoria.
pub const LOOT_CHANCE: i32 = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchResult {
    Win,
    Draw,
    Loss,
}

/// RP guadagnati in una battaglia da ciascuna abilità in addestramento.
pub fn rp_award(unit: &Unit, result: MatchResult) -> i32 {
    RP_BASE
        + if result == MatchResult::Win {
            RP_WIN
        } else {
            0
        }
        + RP_PER_ACTION * unit.tally.actions.min(RP_ACTION_CAP)
}

/// Abilità che l'unità sta imparando con l'equipaggiamento attuale.
pub fn training(class: Class, equipment: &[ItemId], mastered: &[AbilityId]) -> Vec<AbilityId> {
    let mut v: Vec<AbilityId> = equipment
        .iter()
        .filter_map(|i| i.def().teaches)
        .filter(|a| class.learnable().contains(a) && !mastered.contains(a))
        .collect();
    v.sort();
    v.dedup();
    v
}

/// Somma RP a un'abilità; restituisce (nuovi RP, padroneggiata ora?).
pub fn add_rp(ability: AbilityId, current: i32, gained: i32) -> (i32, bool) {
    let max = ability.def().master_rp;
    let rp = (current + gained).min(max);
    (rp, current < max && rp >= max)
}

pub fn coins(result: MatchResult) -> i64 {
    match result {
        MatchResult::Win => COINS_WIN,
        MatchResult::Draw => COINS_DRAW,
        MatchResult::Loss => COINS_LOSS,
    }
}

/// Bottino di fine partita, deterministico dato il seed (verificabile).
/// Gli oggetti più costosi sono più rari.
pub fn loot(seed: u64, result: MatchResult) -> Option<ItemId> {
    loot_with_chance(seed, result, LOOT_CHANCE)
}

/// Come `loot`, con probabilità di bottino personalizzata (es. missioni cooperative).
pub fn loot_with_chance(seed: u64, result: MatchResult, chance: i32) -> Option<ItemId> {
    if result != MatchResult::Win {
        return None;
    }
    let mut rng = Rng::new(seed ^ 0x6c6f_6f74);
    if !rng.percent(chance) {
        return None;
    }
    let weight = |i: ItemId| (2000 / i.def().price.max(50)) as u32;
    let total: u32 = ItemId::ALL.iter().map(|&i| weight(i)).sum();
    let mut roll = rng.below(total);
    for i in ItemId::ALL {
        let w = weight(i);
        if roll < w {
            return Some(i);
        }
        roll -= w;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::starter_kit;

    #[test]
    fn training_only_own_school() {
        let eq = [ItemId::WarPike, ItemId::Leather];
        assert_eq!(training(Class::Vanguard, &eq, &[]), vec![AbilityId::Charge]);
        assert!(training(Class::Warden, &eq, &[]).is_empty());
        assert!(training(Class::Vanguard, &eq, &[AbilityId::Charge]).is_empty());
        assert!(training(Class::Scout, &starter_kit(Class::Scout), &[]).is_empty());
    }

    #[test]
    fn mastery_threshold() {
        let a = AbilityId::Frost; // 80 RP
        assert_eq!(add_rp(a, 50, 20), (70, false));
        assert_eq!(add_rp(a, 70, 20), (80, true));
        assert_eq!(add_rp(a, 80, 20), (80, false));
    }

    #[test]
    fn advanced_class_requirements() {
        use AbilityId::*;
        assert!(Class::Vanguard.unlocked_for(1, &[]));
        assert!(!Class::Runeblade.unlocked_for(9, &[Sweep]));
        assert!(!Class::Runeblade.unlocked_for(4, &[Sweep, Frost]));
        assert!(Class::Runeblade.unlocked_for(5, &[Sweep, Frost]));
        assert!(Class::Warden.unlocked_for(5, &[Charge, Radiance]));
        assert!(!Class::Warden.unlocked_for(5, &[Charge, Frost]));
    }

    #[test]
    fn loot_is_deterministic_and_win_only() {
        assert_eq!(loot(42, MatchResult::Win), loot(42, MatchResult::Win));
        assert_eq!(loot(42, MatchResult::Loss), None);
        let drops = (0..1000)
            .filter(|&s| loot(s, MatchResult::Win).is_some())
            .count();
        assert!((400..600).contains(&drops), "{drops}");
    }
}

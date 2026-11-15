//! Descrizioni testuali (italiano) degli eventi di battaglia, condivise da
//! demo da terminale e client.

use crate::battle::Event;
use crate::unit::UnitId;

/// Descrive un evento; `n` restituisce il nome da mostrare per un'unità.
/// `None` per eventi senza interesse narrativo (inizio turno, movimento nullo).
pub fn describe_event(e: &Event, n: &dyn Fn(UnitId) -> String) -> Option<String> {
    Some(match e {
        Event::TurnStarted { .. } => return None,
        Event::TurnSkipped { unit } => format!("  {} è sospeso e salta il turno", n(*unit)),
        Event::Moved { unit, from, to } if from != to => {
            format!(
                "{} si sposta ({},{}) -> ({},{})",
                n(*unit),
                from.x,
                from.y,
                to.x,
                to.y
            )
        }
        Event::Moved { .. } => return None,
        Event::AbilityUsed {
            unit,
            ability,
            target,
        } => {
            format!(
                "{} usa {} su ({},{})",
                n(*unit),
                ability.def().name,
                target.x,
                target.y
            )
        }
        Event::Damaged {
            unit, amount, hp, ..
        } => {
            format!("  {} subisce {amount} danni (HP {hp})", n(*unit))
        }
        Event::Healed {
            unit, amount, hp, ..
        } => format!("  {} recupera {amount} HP (HP {hp})", n(*unit)),
        Event::Missed { unit, .. } => format!("  mancato {}", n(*unit)),
        Event::Defeated { unit } => format!("  {} è fuori combattimento", n(*unit)),
        Event::TerrainDamage {
            unit,
            terrain,
            amount,
            hp,
        } => {
            format!(
                "  {} subisce {amount} danni da {} (HP {hp})",
                n(*unit),
                terrain.name()
            )
        }
        Event::StatuteViolated { unit, statute } => {
            format!("  ! {} viola lo statuto «{}»", n(*unit), statute.name())
        }
        Event::Suspended { unit, turns } => format!("  ! {} sospeso per {turns} turni", n(*unit)),
        Event::Expelled { unit } => format!("  ! {} espulso dalla battaglia", n(*unit)),
        Event::LevelUp { unit, level } => format!("  {} sale al livello {level}", n(*unit)),
        Event::BattleEnded { .. } => "Battaglia conclusa".to_string(),
    })
}

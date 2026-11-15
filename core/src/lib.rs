//! Motore di regole deterministico del gioco tattico.
//!
//! Usato identico da server (autorità), client (anteprime, replay) e test.

pub mod ability;
pub mod ai;
pub mod battle;
pub mod class;
pub mod item;
pub mod map;
pub mod presets;
pub mod progression;
pub mod rng;
pub mod status;
pub mod statute;
pub mod text;
pub mod unit;

/// Versione delle regole: cambia quando una modifica altera l'esito delle
/// stesse azioni (e quindi la riproducibilità dei replay).
/// 1: versione iniziale; 2: status alterati.
pub const RULES_VERSION: u32 = 2;

pub use battle::{Action, ActionError, Battle, BattleSetup, Event, Objective, Snapshot};
pub use map::{Biome, Dir, Map, MapParams, Pos, Terrain};
pub use unit::{Unit, UnitSetup};

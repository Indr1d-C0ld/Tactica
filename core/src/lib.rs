//! Motore di regole deterministico del gioco tattico.
//!
//! Usato identico da server (autorità), client (anteprime, replay) e test.

pub mod ability;
pub mod ai;
pub mod battle;
pub mod class;
pub mod map;
pub mod rng;
pub mod statute;
pub mod unit;

pub use battle::{Action, ActionError, Battle, BattleSetup, Event};
pub use map::{Biome, Dir, Map, MapParams, Pos, Terrain};
pub use unit::{Unit, UnitSetup};

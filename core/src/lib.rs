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
pub mod statute;
pub mod text;
pub mod unit;

pub use battle::{Action, ActionError, Battle, BattleSetup, Event, Objective, Snapshot};
pub use map::{Biome, Dir, Map, MapParams, Pos, Terrain};
pub use unit::{Unit, UnitSetup};

//! Motore di regole deterministico del gioco tattico.
//!
//! Usato identico da server (autorità), client (anteprime, replay) e test.

pub mod ability;
pub mod ai;
pub mod battle;
pub mod class;
pub mod consumable;
pub mod craft;
pub mod gear;
pub mod item;
pub mod map;
pub mod people;
pub mod presets;
pub mod progression;
pub mod rng;
pub mod status;
pub mod statute;
pub mod text;
pub mod unit;

/// Versione delle regole: cambia quando una modifica altera l'esito delle
/// stesse azioni (e quindi la riproducibilità dei replay).
/// 1: versione iniziale; 2: status alterati; 3: anche gli alleati bloccano il passaggio;
/// 4: bottino dalle unità sconfitte, effetti degli oggetti generati, mappe più grandi con forme del terreno.
pub const RULES_VERSION: u32 = 4;

/// Versione del protocollo client-server (messaggi, snapshot, rotte usate
/// dal client): cambia quando un client vecchio non funzionerebbe più.
pub const PROTOCOL_VERSION: u32 = 1;

/// Versione di compatibilità che client e server devono condividere:
/// `regole.protocollo` (es. "4.1").
pub fn compat_version() -> String {
    format!("{RULES_VERSION}.{PROTOCOL_VERSION}")
}

pub use battle::{
    Action, ActionError, Battle, BattleSetup, Event, LootDrop, LootRules, Objective, Snapshot,
};
pub use map::{Biome, Dir, Map, MapParams, Pos, Terrain};
pub use unit::{Unit, UnitSetup};

//! Messaggi WebSocket (JSON, con campo `type`).
//!
//! Il client invia solo intenzioni; il server risponde con eventi e lo
//! snapshot pubblico aggiornato. Il seed resta segreto fino a fine partita.

use serde::{Deserialize, Serialize};
use tactica_core::{Action, Event, Snapshot};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Contro l'IA del server.
    Pve,
    /// Contro un altro giocatore (classificata, ELO).
    Pvp,
}

impl Mode {
    pub fn key(self) -> &'static str {
        match self {
            Mode::Pve => "pve",
            Mode::Pvp => "pvp",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    /// Entra in coda con le unità del roster indicate (1-4).
    Queue { mode: Mode, units: Vec<i64> },
    /// Esce dalla coda.
    Cancel,
    /// Azione nella partita in corso.
    Action { action: Action },
    /// Abbandona la partita (resa).
    Leave,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Reward {
    pub roster_id: i64,
    pub name: String,
    pub level_before: i64,
    pub level_after: i64,
    pub xp: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    Welcome {
        username: String,
    },
    Queued {
        mode: Mode,
    },
    Matched {
        mode: Mode,
        team: u8,
        opponent: String,
        snapshot: Snapshot,
    },
    Update {
        events: Vec<Event>,
        snapshot: Snapshot,
    },
    Error {
        message: String,
    },
    Ended {
        match_id: i64,
        /// `None` = pareggio.
        winner: Option<u8>,
        result: String,
        elo_before: i64,
        elo_after: i64,
        rewards: Vec<Reward>,
    },
}

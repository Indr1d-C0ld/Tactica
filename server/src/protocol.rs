//! Messaggi WebSocket (JSON, con campo `type`).
//!
//! Il client invia solo intenzioni; il server risponde con eventi e lo
//! snapshot pubblico aggiornato. Il seed resta segreto fino a fine partita.

use serde::{Deserialize, Serialize};
use tactica_core::{Action, Event, Snapshot};

pub use crate::game::MissionInfo;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Contro l'IA del server.
    Pve,
    /// Contro un altro giocatore (classificata, ELO).
    Pvp,
    /// Missione cooperativa di clan contro l'IA.
    Coop,
}

impl Mode {
    pub fn key(self) -> &'static str {
        match self {
            Mode::Pve => "pve",
            Mode::Pvp => "pvp",
            Mode::Coop => "coop",
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
    /// Abbandona la partita (resa; in cooperativa le unità passano all'IA).
    Leave,
    /// Apre una stanza di missione per il proprio clan (difficoltà 1-3).
    RoomCreate { difficulty: u8, units: Vec<i64> },
    /// Entra in una stanza del proprio clan.
    RoomJoin { room: u64, units: Vec<i64> },
    /// Esce dalla stanza.
    RoomLeave,
    /// Il capostanza avvia la missione.
    RoomStart,
    /// Stanze aperte del proprio clan.
    RoomList,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoomMember {
    pub username: String,
    pub units: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoomView {
    pub id: u64,
    pub clan: String,
    pub difficulty: u8,
    pub difficulty_name: String,
    pub host: String,
    pub members: Vec<RoomMember>,
    pub max_players: usize,
    pub min_players: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Reward {
    pub roster_id: i64,
    pub name: String,
    pub level_before: i64,
    pub level_after: i64,
    pub xp: i64,
    /// Punti risonanza guadagnati dalle abilità in addestramento.
    pub rp: Vec<RpGain>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RpGain {
    pub ability: String,
    pub name: String,
    pub gained: i64,
    pub rp: i64,
    pub master_rp: i64,
    /// Padroneggiata con questa partita.
    pub mastered_now: bool,
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
        /// Altri umani nella propria squadra (cooperativa).
        #[serde(default)]
        allies: Vec<String>,
        /// Indici delle unità comandate da questo giocatore.
        #[serde(default)]
        mine: Vec<usize>,
        #[serde(default)]
        mission: Option<MissionInfo>,
        snapshot: Snapshot,
    },
    /// Stato della propria stanza di missione (`None`: non sei in una stanza).
    Room {
        room: Option<RoomView>,
    },
    Rooms {
        rooms: Vec<RoomView>,
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
        coins: i64,
        /// Chiave dell'oggetto ottenuto come bottino.
        loot: Option<String>,
        /// Punteggio stagionale prima/dopo (solo PvP classificato).
        season_before: Option<i64>,
        season_after: Option<i64>,
        /// Punti guadagnati per il proprio clan.
        clan_points: i64,
        rewards: Vec<Reward>,
    },
}

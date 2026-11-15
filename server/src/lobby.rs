//! Coda di matchmaking PvP e canali verso le connessioni.

use tokio::sync::mpsc;

use crate::db::RosterUnit;
use crate::game::MatchInput;
use crate::protocol::ServerMsg;

/// Messaggi dal resto del server verso una connessione WebSocket.
pub enum ToConn {
    Msg(ServerMsg),
    Joined {
        input: mpsc::Sender<MatchInput>,
        team: u8,
    },
}

pub type ConnTx = mpsc::UnboundedSender<ToConn>;

#[derive(Clone)]
pub struct Entrant {
    pub conn_id: u64,
    pub account_id: i64,
    pub username: String,
    pub elo: i64,
    pub units: Vec<RosterUnit>,
    pub conn: ConnTx,
}

impl Entrant {
    pub fn send(&self, msg: ServerMsg) {
        let _ = self.conn.send(ToConn::Msg(msg));
    }
}

/// Per ora una sola coda a un posto: il primo che arriva aspetta il secondo.
#[derive(Default)]
pub struct Lobby {
    waiting: Option<Entrant>,
}

impl Lobby {
    /// Mette in coda; se c'è già un avversario valido restituisce la coppia.
    pub fn enqueue_pvp(&mut self, e: Entrant) -> Option<(Entrant, Entrant)> {
        match self.waiting.take() {
            Some(w) if w.account_id != e.account_id && !w.conn.is_closed() => Some((w, e)),
            _ => {
                self.waiting = Some(e);
                None
            }
        }
    }

    pub fn cancel(&mut self, conn_id: u64) -> bool {
        if self.waiting.as_ref().is_some_and(|w| w.conn_id == conn_id) {
            self.waiting = None;
            true
        } else {
            false
        }
    }
}

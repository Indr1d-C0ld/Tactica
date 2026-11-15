//! Coda PvP, stanze di missione cooperative e canali verso le connessioni.

use std::collections::BTreeMap;

use tactica_core::presets;
use tokio::sync::mpsc;

use crate::db::RosterUnit;
use crate::game::{COOP_SLOTS, MatchInput};
use crate::protocol::{RoomMember, RoomView, ServerMsg};

/// Messaggi dal resto del server verso una connessione WebSocket.
#[allow(clippy::large_enum_variant)]
pub enum ToConn {
    Msg(ServerMsg),
    Joined {
        input: mpsc::Sender<MatchInput>,
        seat: usize,
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

pub const ROOM_MAX_PLAYERS: usize = COOP_SLOTS;

pub struct Room {
    pub id: u64,
    pub clan_id: i64,
    pub clan_tag: String,
    pub difficulty: u8,
    /// Il primo è il capostanza.
    pub members: Vec<Entrant>,
}

impl Room {
    pub fn view(&self, min_players: usize) -> RoomView {
        let quota = (COOP_SLOTS / self.members.len().max(1)).max(1);
        RoomView {
            id: self.id,
            clan: self.clan_tag.clone(),
            difficulty: self.difficulty,
            difficulty_name: presets::difficulty_name(self.difficulty).to_string(),
            host: self
                .members
                .first()
                .map(|m| m.username.clone())
                .unwrap_or_default(),
            members: self
                .members
                .iter()
                .map(|m| RoomMember {
                    username: m.username.clone(),
                    units: m.units.len().min(quota),
                })
                .collect(),
            max_players: ROOM_MAX_PLAYERS,
            min_players,
        }
    }

    /// Notifica lo stato della stanza a tutti i membri.
    pub fn notify(&self, min_players: usize) {
        let v = self.view(min_players);
        for m in &self.members {
            m.send(ServerMsg::Room {
                room: Some(v.clone()),
            });
        }
    }
}

#[derive(Default)]
pub struct Lobby {
    waiting: Option<Entrant>,
    rooms: BTreeMap<u64, Room>,
    next_room: u64,
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

    pub fn room_of(&self, conn_id: u64) -> Option<u64> {
        self.rooms
            .values()
            .find(|r| r.members.iter().any(|m| m.conn_id == conn_id))
            .map(|r| r.id)
    }

    pub fn create_room(
        &mut self,
        e: Entrant,
        clan_id: i64,
        clan_tag: String,
        difficulty: u8,
    ) -> &Room {
        self.next_room += 1;
        let id = self.next_room;
        self.rooms.insert(
            id,
            Room {
                id,
                clan_id,
                clan_tag,
                difficulty: difficulty.clamp(1, presets::MAX_DIFFICULTY),
                members: vec![e],
            },
        );
        &self.rooms[&id]
    }

    pub fn join_room(&mut self, id: u64, e: Entrant, clan_id: i64) -> Result<&Room, &'static str> {
        let room = self.rooms.get_mut(&id).ok_or("stanza non trovata")?;
        if room.clan_id != clan_id {
            return Err("la stanza è di un altro clan");
        }
        if room.members.iter().any(|m| m.account_id == e.account_id) {
            return Err("sei già nella stanza");
        }
        if room.members.len() >= ROOM_MAX_PLAYERS {
            return Err("la stanza è piena");
        }
        room.members.push(e);
        Ok(room)
    }

    /// Toglie la connessione dalla sua stanza; restituisce la stanza se esiste ancora.
    pub fn leave_room(&mut self, conn_id: u64) -> Option<&Room> {
        let id = self.room_of(conn_id)?;
        let room = self.rooms.get_mut(&id)?;
        room.members.retain(|m| m.conn_id != conn_id);
        if room.members.is_empty() {
            self.rooms.remove(&id);
            return None;
        }
        self.rooms.get(&id)
    }

    /// Avvio: solo il capostanza, con abbastanza giocatori. Rimuove la stanza.
    pub fn take_room(&mut self, conn_id: u64, min_players: usize) -> Result<Room, &'static str> {
        let id = self.room_of(conn_id).ok_or("non sei in una stanza")?;
        let room = &self.rooms[&id];
        if room.members[0].conn_id != conn_id {
            return Err("solo il capostanza può avviare la missione");
        }
        if room.members.len() < min_players {
            return Err("servono altri giocatori per avviare la missione");
        }
        Ok(self.rooms.remove(&id).expect("stanza"))
    }

    pub fn rooms_for_clan(&self, clan_id: i64, min_players: usize) -> Vec<RoomView> {
        self.rooms
            .values()
            .filter(|r| r.clan_id == clan_id)
            .map(|r| r.view(min_players))
            .collect()
    }
}

//! Coda PvP, stanze di missione cooperative e canali verso le connessioni.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use tactica_core::presets;
use tokio::sync::mpsc;

use crate::db::RosterUnit;
use crate::game::{COOP_SLOTS, MatchInput};
use crate::protocol::{RoomMember, RoomView, ServerMsg, WarRoomView, WarSide};
use crate::wars::WarInfo;

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

/// Schieramento di una guerra: un lato per clan.
pub struct WarRoom {
    pub info: WarInfo,
    pub sides: [Vec<Entrant>; 2],
    pub ready: [bool; 2],
}

impl WarRoom {
    pub fn view(&self, min_players: usize) -> WarRoomView {
        let side = |i: usize| {
            let quota = (COOP_SLOTS / self.sides[i].len().max(1)).max(1);
            WarSide {
                tag: self.info.tags[i].clone(),
                name: self.info.names[i].clone(),
                members: self.sides[i]
                    .iter()
                    .map(|m| RoomMember {
                        username: m.username.clone(),
                        units: m.units.len().min(quota),
                    })
                    .collect(),
                ready: self.ready[i],
            }
        };
        WarRoomView {
            war: self.info.id,
            sides: vec![side(0), side(1)],
            min_players,
        }
    }

    pub fn notify(&self, min_players: usize) {
        let v = self.view(min_players);
        for m in self.sides.iter().flatten() {
            m.send(ServerMsg::WarRoom {
                room: Some(v.clone()),
            });
        }
    }

    fn side_of(&self, conn_id: u64) -> Option<usize> {
        (0..2).find(|&i| self.sides[i].iter().any(|m| m.conn_id == conn_id))
    }
}

/// Scarto di ELO accettato da chi è appena entrato in coda.
pub const ELO_WINDOW_START: i64 = 100;
/// Di quanto si allarga lo scarto ogni 10 secondi d'attesa.
pub const ELO_WINDOW_STEP: i64 = 50;
/// Dopo questa attesa si accetta qualsiasi avversario (server con pochi giocatori).
pub const ELO_ANY_AFTER: Duration = Duration::from_secs(90);

/// Scarto di ELO accettato dopo un'attesa `waited`.
pub fn elo_window(waited: Duration) -> i64 {
    if waited >= ELO_ANY_AFTER {
        i64::MAX
    } else {
        ELO_WINDOW_START + ELO_WINDOW_STEP * (waited.as_secs() / 10) as i64
    }
}

struct Queued {
    e: Entrant,
    since: Instant,
}

#[derive(Default)]
pub struct Lobby {
    /// Coda PvP: ognuno accetta avversari entro uno scarto di ELO che cresce con l'attesa.
    waiting: Vec<Queued>,
    rooms: BTreeMap<u64, Room>,
    next_room: u64,
    wars: BTreeMap<i64, WarRoom>,
}

impl Lobby {
    /// Mette in coda; se c'è già un avversario compatibile restituisce la coppia
    /// (prima chi aspettava, poi il nuovo arrivato).
    pub fn enqueue_pvp(&mut self, e: Entrant, now: Instant) -> Option<(Entrant, Entrant)> {
        self.waiting
            .retain(|q| !q.e.conn.is_closed() && q.e.account_id != e.account_id);
        let fresh = elo_window(Duration::ZERO);
        let best = self
            .waiting
            .iter()
            .enumerate()
            .filter(|(_, q)| {
                let gap = (q.e.elo - e.elo).abs();
                gap <= fresh.max(elo_window(now.duration_since(q.since)))
            })
            .min_by_key(|(_, q)| ((q.e.elo - e.elo).abs(), q.since))
            .map(|(i, _)| i);
        match best {
            Some(i) => Some((self.waiting.remove(i).e, e)),
            None => {
                self.waiting.push(Queued { e, since: now });
                None
            }
        }
    }

    /// Riesamina la coda (lo scarto accettato cresce col tempo): restituisce le
    /// coppie ora compatibili, servendo per primi quelli che aspettano da più tempo.
    pub fn match_pvp(&mut self, now: Instant) -> Vec<(Entrant, Entrant)> {
        self.waiting.retain(|q| !q.e.conn.is_closed());
        self.waiting.sort_by_key(|q| q.since);
        let mut pairs = Vec::new();
        let mut i = 0;
        while i < self.waiting.len() {
            let a = &self.waiting[i];
            let wa = elo_window(now.duration_since(a.since));
            let best = self
                .waiting
                .iter()
                .enumerate()
                .skip(i + 1)
                .filter(|(_, b)| {
                    b.e.account_id != a.e.account_id
                        && (b.e.elo - a.e.elo).abs()
                            <= wa.max(elo_window(now.duration_since(b.since)))
                })
                .min_by_key(|(_, b)| (b.e.elo - a.e.elo).abs())
                .map(|(j, _)| j);
            match best {
                Some(j) => {
                    let b = self.waiting.remove(j).e;
                    let a = self.waiting.remove(i).e;
                    pairs.push((a, b));
                }
                None => i += 1,
            }
        }
        pairs
    }

    pub fn queued_pvp(&self) -> usize {
        self.waiting.len()
    }

    pub fn cancel(&mut self, conn_id: u64) -> bool {
        let before = self.waiting.len();
        self.waiting.retain(|q| q.e.conn_id != conn_id);
        self.waiting.len() != before
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

    pub fn war_room_of(&self, conn_id: u64) -> Option<i64> {
        self.wars
            .values()
            .find(|w| w.side_of(conn_id).is_some())
            .map(|w| w.info.id)
    }

    /// Ci si può impegnare in una sola cosa alla volta: coda, stanza o schieramento.
    pub fn is_engaged(&self, conn_id: u64) -> bool {
        self.room_of(conn_id).is_some()
            || self.war_room_of(conn_id).is_some()
            || self.waiting.iter().any(|q| q.e.conn_id == conn_id)
    }

    pub fn join_war(
        &mut self,
        info: WarInfo,
        side: usize,
        e: Entrant,
    ) -> Result<&WarRoom, &'static str> {
        let id = info.id;
        let room = self.wars.entry(id).or_insert_with(|| WarRoom {
            info,
            sides: [Vec::new(), Vec::new()],
            ready: [false; 2],
        });
        if room.sides[side]
            .iter()
            .any(|m| m.account_id == e.account_id)
        {
            return Err("sei già schierato");
        }
        if room.sides[side].len() >= COOP_SLOTS {
            return Err("lo schieramento è al completo");
        }
        room.sides[side].push(e);
        room.ready = [false; 2];
        Ok(room)
    }

    pub fn leave_war(&mut self, conn_id: u64) -> Option<&WarRoom> {
        let id = self.war_room_of(conn_id)?;
        let room = self.wars.get_mut(&id)?;
        for side in room.sides.iter_mut() {
            side.retain(|m| m.conn_id != conn_id);
        }
        room.ready = [false; 2];
        if room.sides.iter().all(Vec::is_empty) {
            self.wars.remove(&id);
            return None;
        }
        self.wars.get(&id)
    }

    /// Imposta "pronti" per il lato di chi chiama; se entrambi i lati sono
    /// pronti e abbastanza numerosi, rimuove e restituisce lo schieramento.
    pub fn war_ready(
        &mut self,
        conn_id: u64,
        ready: bool,
        min_players: usize,
    ) -> Result<Option<WarRoom>, &'static str> {
        let id = self.war_room_of(conn_id).ok_or("non sei schierato")?;
        let room = self.wars.get_mut(&id).expect("schieramento");
        let side = room.side_of(conn_id).expect("lato");
        if ready && room.sides[side].len() < min_players {
            return Err("servono altri membri nel tuo schieramento");
        }
        room.ready[side] = ready;
        let go = room.ready.iter().all(|&r| r) && room.sides.iter().all(|s| s.len() >= min_players);
        Ok(if go { self.wars.remove(&id) } else { None })
    }

    pub fn war_room(&self, id: i64) -> Option<&WarRoom> {
        self.wars.get(&id)
    }

    pub fn close_war_room(&mut self, id: i64) -> Option<WarRoom> {
        self.wars.remove(&id)
    }
}

#[cfg(test)]
mod pvp_tests {
    use super::*;

    fn entrant(conn_id: u64, elo: i64) -> (Entrant, mpsc::UnboundedReceiver<ToConn>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (
            Entrant {
                conn_id,
                account_id: conn_id as i64,
                username: format!("p{conn_id}"),
                elo,
                units: Vec::new(),
                conn: tx,
            },
            rx,
        )
    }

    #[test]
    fn window_grows_with_wait() {
        assert_eq!(elo_window(Duration::ZERO), 100);
        assert_eq!(elo_window(Duration::from_secs(25)), 200);
        assert_eq!(elo_window(ELO_ANY_AFTER), i64::MAX);
    }

    #[test]
    fn pairs_close_ratings_first() {
        let t0 = Instant::now();
        let mut l = Lobby::default();
        let (a, _ra) = entrant(1, 1000);
        let (b, _rb) = entrant(2, 1400);
        let (c, _rc) = entrant(3, 1060);
        assert!(l.enqueue_pvp(a, t0).is_none());
        // 400 punti di scarto: troppi per chi è appena arrivato.
        assert!(l.enqueue_pvp(b, t0).is_none());
        let (x, y) = l.enqueue_pvp(c, t0).expect("1000 e 1060 sono vicini");
        assert_eq!((x.conn_id, y.conn_id), (1, 3));
        assert_eq!(l.queued_pvp(), 1);
    }

    #[test]
    fn wide_gap_matches_after_waiting() {
        let t0 = Instant::now();
        let mut l = Lobby::default();
        let (a, _ra) = entrant(1, 1000);
        let (b, _rb) = entrant(2, 1250);
        l.enqueue_pvp(a, t0);
        l.enqueue_pvp(b, t0);
        assert!(l.match_pvp(t0 + Duration::from_secs(10)).is_empty());
        // Dopo 30 s lo scarto accettato è 250.
        let pairs = l.match_pvp(t0 + Duration::from_secs(30));
        assert_eq!(pairs.len(), 1);
        assert_eq!(l.queued_pvp(), 0);
    }

    #[test]
    fn closed_and_cancelled_leave_the_queue() {
        let t0 = Instant::now();
        let mut l = Lobby::default();
        let (a, ra) = entrant(1, 1000);
        let (b, _rb) = entrant(2, 1000);
        l.enqueue_pvp(a, t0);
        drop(ra);
        assert!(
            l.enqueue_pvp(b, t0).is_none(),
            "chi ha chiuso la connessione non gioca"
        );
        assert!(l.is_engaged(2));
        assert!(l.cancel(2));
        assert_eq!(l.queued_pvp(), 0);
    }
}

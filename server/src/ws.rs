//! Connessione WebSocket di un giocatore autenticato.

use std::sync::atomic::Ordering;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::Response;
use serde::Deserialize;
use tokio::sync::mpsc;

use crate::AppState;
use crate::auth::{AuthUser, user_for_token};
use crate::error::{ApiError, ApiResult};
use crate::game::{self, InputKind, MatchInput, Seat};
use crate::lobby::{Entrant, ToConn};
use crate::protocol::{ClientMsg, Mode, ServerMsg};
use crate::{clans, db, wars};

pub const MAX_PARTY: usize = 4;

#[derive(Deserialize)]
pub struct WsQuery {
    pub token: String,
}

pub async fn upgrade(
    ws: WebSocketUpgrade,
    Query(q): Query<WsQuery>,
    State(state): State<AppState>,
) -> ApiResult<Response> {
    let user = user_for_token(&state.db, &q.token)
        .await?
        .ok_or_else(ApiError::unauthorized)?;
    Ok(ws.on_upgrade(move |socket| connection(socket, state, user)))
}

struct Conn {
    id: u64,
    user: AuthUser,
    tx: mpsc::UnboundedSender<ToConn>,
    /// Partita in corso: canale di input e posto del giocatore.
    current: Option<(mpsc::Sender<MatchInput>, usize)>,
}

impl Conn {
    fn send(&self, msg: ServerMsg) {
        let _ = self.tx.send(ToConn::Msg(msg));
    }

    fn error(&self, message: impl Into<String>) {
        self.send(ServerMsg::Error {
            message: message.into(),
        });
    }
}

async fn connection(mut socket: WebSocket, state: AppState, user: AuthUser) {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut conn = Conn {
        id: state.next_conn.fetch_add(1, Ordering::Relaxed),
        user,
        tx,
        current: None,
    };
    tracing::debug!(user = %conn.user.username, conn = conn.id, "connesso");
    state.online.lock().unwrap().insert(
        conn.id,
        crate::Online {
            account_id: conn.user.id,
            username: conn.user.username.clone(),
            since: crate::db::now(),
            tx: conn.tx.clone(),
        },
    );
    conn.send(ServerMsg::Welcome {
        username: conn.user.username.clone(),
    });

    loop {
        tokio::select! {
            incoming = socket.recv() => {
                let text = match incoming {
                    Some(Ok(Message::Text(t))) => t,
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    Some(Ok(_)) => continue,
                };
                match serde_json::from_str::<ClientMsg>(text.as_str()) {
                    Ok(msg) => handle(&state, &mut conn, msg).await,
                    Err(e) => conn.error(format!("messaggio non valido: {e}")),
                }
            }
            Some(out) = rx.recv() => {
                let msg = match out {
                    ToConn::Joined { input, seat, .. } => {
                        conn.current = Some((input, seat));
                        continue;
                    }
                    ToConn::Msg(m) => m,
                    ToConn::Kick => break,
                };
                if matches!(msg, ServerMsg::Ended { .. }) {
                    conn.current = None;
                }
                let json = serde_json::to_string(&msg).expect("serializzazione");
                if socket.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
        }
    }

    state.online.lock().unwrap().remove(&conn.id);
    leave_lobby(&state, conn.id);
    if let Some((input, seat)) = conn.current.take() {
        let _ = input
            .send(MatchInput {
                seat,
                kind: InputKind::Disconnected,
            })
            .await;
    }
    tracing::debug!(user = %conn.user.username, conn = conn.id, "disconnesso");
}

/// Esce da coda e stanza, avvisando gli altri membri della stanza.
fn leave_lobby(state: &AppState, conn_id: u64) {
    let mut lobby = state.lobby.lock().unwrap();
    lobby.cancel(conn_id);
    if let Some(room) = lobby.leave_room(conn_id) {
        room.notify(state.config.coop_min_players);
    }
    if let Some(room) = lobby.leave_war(conn_id) {
        room.notify(state.config.war_min_players);
    }
}

async fn handle(state: &AppState, conn: &mut Conn, msg: ClientMsg) {
    // In manutenzione solo gli admin avviano nuove partite.
    let starts = matches!(
        msg,
        ClientMsg::Queue { .. }
            | ClientMsg::RoomCreate { .. }
            | ClientMsg::RoomJoin { .. }
            | ClientMsg::WarJoin { .. }
            | ClientMsg::Campaign { .. }
    );
    if starts && !conn.user.is_admin && state.settings.read().unwrap().maintenance {
        return conn.error("server in manutenzione: le partite riprenderanno a breve");
    }
    match msg {
        ClientMsg::Queue { mode, units } => queue(state, conn, mode, units).await,
        ClientMsg::Cancel => {
            state.lobby.lock().unwrap().cancel(conn.id);
        }
        ClientMsg::Action { action } => {
            let Some((input, seat)) = &conn.current else {
                return conn.error("nessuna partita in corso");
            };
            let _ = input
                .send(MatchInput {
                    seat: *seat,
                    kind: InputKind::Action(action),
                })
                .await;
        }
        ClientMsg::Leave => {
            if let Some((input, seat)) = &conn.current {
                let _ = input
                    .send(MatchInput {
                        seat: *seat,
                        kind: InputKind::Leave,
                    })
                    .await;
            }
        }
        ClientMsg::RoomCreate { difficulty, units } => {
            room_enter(state, conn, None, difficulty, units).await
        }
        ClientMsg::RoomJoin { room, units } => room_enter(state, conn, Some(room), 0, units).await,
        ClientMsg::RoomLeave => {
            leave_lobby(state, conn.id);
            conn.send(ServerMsg::Room { room: None });
        }
        ClientMsg::RoomStart => {
            let taken = state
                .lobby
                .lock()
                .unwrap()
                .take_room(conn.id, state.config.coop_min_players);
            match taken {
                Ok(room) => {
                    // Chi nel frattempo è entrato in un'altra partita resta fuori.
                    let busy = state.busy.lock().unwrap().clone();
                    let members: Vec<Entrant> = room
                        .members
                        .into_iter()
                        .filter(|m| !busy.contains(&m.account_id))
                        .collect();
                    for m in &members {
                        m.send(ServerMsg::Room { room: None });
                    }
                    game::start_mission(state.clone(), members, room.difficulty);
                }
                Err(e) => conn.error(e),
            }
        }
        ClientMsg::WarJoin { war, units } => war_join(state, conn, war, units).await,
        ClientMsg::WarLeave => {
            let mut lobby = state.lobby.lock().unwrap();
            if let Some(room) = lobby.leave_war(conn.id) {
                room.notify(state.config.war_min_players);
            }
            conn.send(ServerMsg::WarRoom { room: None });
        }
        ClientMsg::WarReady { ready } => war_ready(state, conn, ready).await,
        ClientMsg::Campaign { node, units } => {
            let e = match entrant(state, conn, &units).await {
                Ok(e) => e,
                Err(m) => return conn.error(m),
            };
            if state.lobby.lock().unwrap().is_engaged(conn.id) {
                return conn.error("esci prima dalla coda, dalla stanza o dallo schieramento");
            }
            let mut c = match state.db.acquire().await {
                Ok(c) => c,
                Err(_) => return conn.error("errore interno"),
            };
            match crate::campaign::unlocked(&mut c, conn.user.id, &node).await {
                Ok(true) => game::start_campaign(state.clone(), e, &node),
                Ok(false) => conn.error("tappa non ancora sbloccata"),
                Err(_) => conn.error("errore interno"),
            }
        }
        ClientMsg::RoomList => {
            let mut c = match state.db.acquire().await {
                Ok(c) => c,
                Err(_) => return conn.error("errore interno"),
            };
            let Ok(Some(m)) = clans::membership(&mut c, conn.user.id).await else {
                return conn.send(ServerMsg::Rooms { rooms: Vec::new() });
            };
            let rooms = state
                .lobby
                .lock()
                .unwrap()
                .rooms_for_clan(m.clan_id, state.config.coop_min_players);
            conn.send(ServerMsg::Rooms { rooms });
        }
    }
}

/// Unità del roster scelte per una partita, validate.
async fn entrant(state: &AppState, conn: &Conn, ids: &[i64]) -> Result<Entrant, String> {
    if conn.current.is_some() || state.busy.lock().unwrap().contains(&conn.user.id) {
        return Err("sei già in partita".into());
    }
    let mut unique = ids.to_vec();
    unique.sort_unstable();
    unique.dedup();
    if ids.is_empty() || ids.len() > MAX_PARTY || unique.len() != ids.len() {
        return Err(format!("scegli da 1 a {MAX_PARTY} unità diverse"));
    }
    let units = match db::roster_units(&state.db, conn.user.id, ids).await {
        Ok(Some(u)) => u,
        Ok(None) => return Err("unità non presenti nel tuo roster".into()),
        Err(e) => {
            tracing::error!("roster: {e}");
            return Err("errore interno".into());
        }
    };
    let elo = sqlx::query_scalar("SELECT elo FROM accounts WHERE id = ?")
        .bind(conn.user.id)
        .fetch_one(&state.db)
        .await
        .unwrap_or(1000);
    Ok(Entrant {
        conn_id: conn.id,
        account_id: conn.user.id,
        username: conn.user.username.clone(),
        elo,
        units,
        conn: conn.tx.clone(),
    })
}

async fn room_enter(
    state: &AppState,
    conn: &mut Conn,
    room: Option<u64>,
    difficulty: u8,
    ids: Vec<i64>,
) {
    let e = match entrant(state, conn, &ids).await {
        Ok(e) => e,
        Err(m) => return conn.error(m),
    };
    let mut c = match state.db.acquire().await {
        Ok(c) => c,
        Err(_) => return conn.error("errore interno"),
    };
    let Ok(Some(m)) = clans::membership(&mut c, conn.user.id).await else {
        return conn.error("le missioni cooperative sono per i membri di un clan");
    };
    let min = state.config.coop_min_players;
    let mut lobby = state.lobby.lock().unwrap();
    lobby.cancel(conn.id);
    if lobby.is_engaged(conn.id) {
        return conn.error("sei già in una stanza o in uno schieramento");
    }
    match room {
        None => lobby
            .create_room(e, m.clan_id, m.tag, difficulty)
            .notify(min),
        Some(id) => match lobby.join_room(id, e, m.clan_id) {
            Ok(r) => r.notify(min),
            Err(msg) => conn.error(msg),
        },
    }
}

async fn queue(state: &AppState, conn: &mut Conn, mode: Mode, ids: Vec<i64>) {
    let entrant = match entrant(state, conn, &ids).await {
        Ok(e) => e,
        Err(m) => return conn.error(m),
    };
    {
        let lobby = state.lobby.lock().unwrap();
        if lobby.room_of(conn.id).is_some() || lobby.war_room_of(conn.id).is_some() {
            return conn.error("esci prima dalla stanza o dallo schieramento");
        }
    }
    match mode {
        Mode::Pve => game::start_match(state.clone(), mode, [Seat::Human(entrant), Seat::Ai]),
        Mode::Pvp => {
            let pair = state
                .lobby
                .lock()
                .unwrap()
                .enqueue_pvp(entrant, std::time::Instant::now());
            match pair {
                Some((a, b)) => {
                    game::start_match(state.clone(), mode, [Seat::Human(a), Seat::Human(b)])
                }
                None => conn.send(ServerMsg::Queued { mode }),
            }
        }
        Mode::Coop => conn.error("le missioni si avviano da una stanza di clan"),
        Mode::War => conn.error("le guerre si combattono dallo schieramento del clan"),
        Mode::Campaign => conn.error("le tappe della campagna si avviano dalla mappa del mondo"),
    }
}

async fn war_join(state: &AppState, conn: &mut Conn, war: i64, ids: Vec<i64>) {
    let e = match entrant(state, conn, &ids).await {
        Ok(e) => e,
        Err(m) => return conn.error(m),
    };
    let mut c = match state.db.acquire().await {
        Ok(c) => c,
        Err(_) => return conn.error("errore interno"),
    };
    let (info, side, _role) = match wars::for_join(&mut c, war, conn.user.id).await {
        Ok(v) => v,
        Err(m) => return conn.error(m),
    };
    let min = state.config.war_min_players;
    let mut lobby = state.lobby.lock().unwrap();
    lobby.cancel(conn.id);
    if lobby.is_engaged(conn.id) {
        return conn.error("sei già in una stanza o in uno schieramento");
    }
    match lobby.join_war(info, side, e) {
        Ok(room) => room.notify(min),
        Err(m) => conn.error(m),
    }
}

async fn war_ready(state: &AppState, conn: &mut Conn, ready: bool) {
    let war = state.lobby.lock().unwrap().war_room_of(conn.id);
    let Some(war) = war else {
        return conn.error("non sei schierato");
    };
    let mut c = match state.db.acquire().await {
        Ok(c) => c,
        Err(_) => return conn.error("errore interno"),
    };
    match wars::for_join(&mut c, war, conn.user.id).await {
        Ok((_, _, role)) if role != "member" => {}
        Ok(_) => return conn.error("solo capo e ufficiali danno il via"),
        Err(m) => return conn.error(m),
    }
    let min = state.config.war_min_players;
    let started = {
        let mut lobby = state.lobby.lock().unwrap();
        match lobby.war_ready(conn.id, ready, min) {
            Ok(Some(room)) => Some(room),
            Ok(None) => {
                if let Some(r) = lobby.war_room(war) {
                    r.notify(min);
                }
                None
            }
            Err(m) => return conn.error(m),
        }
    };
    if let Some(room) = started {
        let busy = state.busy.lock().unwrap().clone();
        let sides = room.sides.map(|side| {
            side.into_iter()
                .filter(|m| !busy.contains(&m.account_id))
                .collect::<Vec<_>>()
        });
        if sides.iter().any(Vec::is_empty) {
            return conn.error("uno schieramento è rimasto vuoto");
        }
        for m in sides.iter().flatten() {
            m.send(ServerMsg::WarRoom { room: None });
        }
        game::start_war(state.clone(), room.info, sides);
    }
}

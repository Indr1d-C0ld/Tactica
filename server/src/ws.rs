//! Connessione WebSocket di un giocatore autenticato.

use std::sync::atomic::Ordering;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::Response;
use serde::Deserialize;
use tactica_core::Action;
use tokio::sync::mpsc;

use crate::AppState;
use crate::auth::{AuthUser, user_for_token};
use crate::db;
use crate::error::{ApiError, ApiResult};
use crate::game::{self, InputKind, MatchInput, Seat};
use crate::lobby::{Entrant, ToConn};
use crate::protocol::{ClientMsg, Mode, ServerMsg};

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
    current: Option<(mpsc::Sender<MatchInput>, u8)>,
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
                    ToConn::Joined { input, team } => {
                        conn.current = Some((input, team));
                        continue;
                    }
                    ToConn::Msg(m) => m,
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

    state.lobby.lock().unwrap().cancel(conn.id);
    if let Some((input, team)) = conn.current.take() {
        let _ = input
            .send(MatchInput {
                team,
                kind: InputKind::Disconnected,
            })
            .await;
    }
    tracing::debug!(user = %conn.user.username, conn = conn.id, "disconnesso");
}

async fn handle(state: &AppState, conn: &mut Conn, msg: ClientMsg) {
    match msg {
        ClientMsg::Queue { mode, units } => queue(state, conn, mode, units).await,
        ClientMsg::Cancel => {
            state.lobby.lock().unwrap().cancel(conn.id);
        }
        ClientMsg::Action { action } => {
            let Some((input, team)) = &conn.current else {
                return conn.error("nessuna partita in corso");
            };
            // Si può arrendere solo la propria squadra.
            let action = match action {
                Action::Concede(_) => Action::Concede(*team),
                a => a,
            };
            let _ = input
                .send(MatchInput {
                    team: *team,
                    kind: InputKind::Action(action),
                })
                .await;
        }
        ClientMsg::Leave => {
            if let Some((input, team)) = &conn.current {
                let _ = input
                    .send(MatchInput {
                        team: *team,
                        kind: InputKind::Action(Action::Concede(*team)),
                    })
                    .await;
            }
        }
    }
}

async fn queue(state: &AppState, conn: &mut Conn, mode: Mode, ids: Vec<i64>) {
    if conn.current.is_some() || state.busy.lock().unwrap().contains(&conn.user.id) {
        return conn.error("sei già in partita");
    }
    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    if ids.is_empty() || ids.len() > MAX_PARTY || unique.len() != ids.len() {
        return conn.error(format!("scegli da 1 a {MAX_PARTY} unità diverse"));
    }
    let units = match db::roster_units(&state.db, conn.user.id, &ids).await {
        Ok(Some(u)) => u,
        Ok(None) => return conn.error("unità non presenti nel tuo roster"),
        Err(e) => {
            tracing::error!("roster: {e}");
            return conn.error("errore interno");
        }
    };
    let elo = sqlx::query_scalar("SELECT elo FROM accounts WHERE id = ?")
        .bind(conn.user.id)
        .fetch_one(&state.db)
        .await
        .unwrap_or(1000);
    let entrant = Entrant {
        conn_id: conn.id,
        account_id: conn.user.id,
        username: conn.user.username.clone(),
        elo,
        units,
        conn: conn.tx.clone(),
    };
    match mode {
        Mode::Pve => game::start_match(state.clone(), mode, [Seat::Human(entrant), Seat::Ai]),
        Mode::Pvp => {
            let pair = state.lobby.lock().unwrap().enqueue_pvp(entrant);
            match pair {
                Some((a, b)) => {
                    game::start_match(state.clone(), mode, [Seat::Human(a), Seat::Human(b)])
                }
                None => conn.send(ServerMsg::Queued { mode }),
            }
        }
    }
}

//! Server autoritativo: account, roster persistente, partite PvE/PvP via
//! WebSocket, replay e statistiche personali/globali su SQLite (WAL).

pub mod api;
pub mod armory;
pub mod auth;
pub mod campaign;
pub mod clans;
pub mod config;
pub mod db;
pub mod error;
pub mod game;
pub mod lobby;
pub mod protocol;
pub mod ratelimit;
pub mod seasons;
pub mod stats;
pub mod tournaments;
pub mod wars;
pub mod ws;

use std::collections::HashSet;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use axum::Router;
use sqlx::SqlitePool;

pub use config::Config;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub config: Arc<Config>,
    pub lobby: Arc<Mutex<lobby::Lobby>>,
    /// Account attualmente in partita (un roster non può giocare due partite insieme).
    pub busy: Arc<Mutex<HashSet<i64>>>,
    pub next_conn: Arc<AtomicU64>,
    pub limiter: Arc<ratelimit::Limiter>,
}

impl AppState {
    pub async fn new(config: Config) -> anyhow::Result<Self> {
        let db = db::connect(&config.db_url).await?;
        wars::recover(&db).await?;
        let state = AppState {
            db,
            config: Arc::new(config),
            lobby: Arc::default(),
            busy: Arc::default(),
            next_conn: Arc::new(AtomicU64::new(1)),
            limiter: Arc::default(),
        };
        state.spawn_matchmaker();
        Ok(state)
    }

    /// Ogni 2 secondi riesamina la coda PvP: lo scarto di ELO accettato cresce
    /// con l'attesa, quindi coppie prima incompatibili possono diventarlo.
    fn spawn_matchmaker(&self) {
        let weak = Arc::downgrade(&self.lobby);
        let state = self.clone();
        // Il compito tiene solo un riferimento debole alla lobby per non
        // tenere in vita lo stato; per avviare le partite gli serve lo stato
        // completo, che ricostruisce finché la lobby esiste.
        let AppState {
            db,
            config,
            busy,
            next_conn,
            limiter,
            ..
        } = state;
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(2));
            loop {
                tick.tick().await;
                let Some(lobby) = weak.upgrade() else { break };
                let pairs = lobby.lock().unwrap().match_pvp(std::time::Instant::now());
                if pairs.is_empty() {
                    continue;
                }
                let st = AppState {
                    db: db.clone(),
                    config: config.clone(),
                    lobby,
                    busy: busy.clone(),
                    next_conn: next_conn.clone(),
                    limiter: limiter.clone(),
                };
                for (a, b) in pairs {
                    game::start_match(
                        st.clone(),
                        protocol::Mode::Pvp,
                        [game::Seat::Human(a), game::Seat::Human(b)],
                    );
                }
            }
        });
    }
}

pub fn router(state: AppState) -> Router {
    api::routes().with_state(state)
}

//! Server autoritativo: account, roster persistente, partite PvE/PvP via
//! WebSocket, replay e statistiche personali/globali su SQLite (WAL).

pub mod api;
pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod game;
pub mod lobby;
pub mod protocol;
pub mod stats;
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
}

impl AppState {
    pub async fn new(config: Config) -> anyhow::Result<Self> {
        let db = db::connect(&config.db_url).await?;
        Ok(AppState {
            db,
            config: Arc::new(config),
            lobby: Arc::default(),
            busy: Arc::default(),
            next_conn: Arc::new(AtomicU64::new(1)),
        })
    }
}

pub fn router(state: AppState) -> Router {
    api::routes().with_state(state)
}

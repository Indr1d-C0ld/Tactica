use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{FromRow, SqlitePool};
use tactica_core::class::Class;

pub const MAX_ROSTER: i64 = 8;

pub async fn connect(url: &str) -> anyhow::Result<SqlitePool> {
    let opts = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5));
    // Un database in memoria esiste per connessione: una sola connessione condivisa.
    let max = if url.contains(":memory:") { 1 } else { 8 };
    let pool = SqlitePoolOptions::new()
        .max_connections(max)
        .connect_with(opts)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

/// Copia coerente del database (anche a server attivo) con rotazione.
pub async fn backup(db: &SqlitePool, dir: &str, keep: usize) -> anyhow::Result<String> {
    tokio::fs::create_dir_all(dir).await?;
    let path = format!("{dir}/tactica-{}.db", now());
    sqlx::query("VACUUM INTO ?").bind(&path).execute(db).await?;
    let mut files = Vec::new();
    let mut rd = tokio::fs::read_dir(dir).await?;
    while let Some(e) = rd.next_entry().await? {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with("tactica-") && name.ends_with(".db") {
            files.push(e.path());
        }
    }
    files.sort();
    let excess = files.len().saturating_sub(keep.max(1));
    for old in &files[..excess] {
        tokio::fs::remove_file(old).await?;
    }
    Ok(path)
}

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

#[derive(Clone, Debug, Serialize, FromRow, PartialEq, Eq)]
pub struct RosterUnit {
    pub id: i64,
    pub name: String,
    pub class: String,
    pub level: i64,
    pub xp: i64,
}

impl RosterUnit {
    pub fn class(&self) -> Class {
        Class::from_key(&self.class).unwrap_or(Class::Vanguard)
    }
}

pub async fn roster(db: &SqlitePool, account: i64) -> sqlx::Result<Vec<RosterUnit>> {
    sqlx::query_as("SELECT id, name, class, level, xp FROM roster WHERE account_id = ? ORDER BY id")
        .bind(account)
        .fetch_all(db)
        .await
}

/// Unità del roster richieste, nell'ordine dato; `None` se qualcuna non appartiene all'account.
pub async fn roster_units(
    db: &SqlitePool,
    account: i64,
    ids: &[i64],
) -> sqlx::Result<Option<Vec<RosterUnit>>> {
    let all = roster(db, account).await?;
    Ok(ids
        .iter()
        .map(|id| all.iter().find(|u| u.id == *id).cloned())
        .collect())
}

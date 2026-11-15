//! Stagioni classificate.
//!
//! - Ogni stagione dura `season_days`; ne è sempre attiva una (creata al bisogno).
//! - Il PvP aggiorna, oltre all'ELO di carriera, un punteggio stagionale.
//!   Alla prima partita di una stagione il punteggio parte da un "reset morbido":
//!   1000 + (punteggio della stagione precedente - 1000) / 2.
//! - I clan accumulano punti stagionali con le partite dei membri.
//! - A scadenza la stagione viene chiusa: classifiche finali, titoli e monete.

use serde::Serialize;
use sqlx::{SqliteConnection, SqlitePool};

use crate::db::now;

pub const BASE_RATING: i64 = 1000;
pub const CLAN_POINTS_PVP_WIN: i64 = 3;
pub const CLAN_POINTS_PVP_DRAW: i64 = 1;
pub const CLAN_POINTS_PVE_WIN: i64 = 1;

const NAMES: [&str; 6] = [
    "dell'Alba",
    "delle Braci",
    "del Gelo",
    "delle Maree",
    "dei Venti",
    "della Cenere",
];

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct Season {
    pub id: i64,
    pub number: i64,
    pub name: String,
    pub starts_at: i64,
    pub ends_at: i64,
    pub closed: bool,
}

pub fn season_name(number: i64) -> String {
    format!(
        "Stagione {number} {}",
        NAMES[((number - 1).max(0) as usize) % NAMES.len()]
    )
}

/// Stagione attiva; se non ce n'è una la crea (a partire da adesso).
pub async fn current(conn: &mut SqliteConnection, days: i64) -> sqlx::Result<Season> {
    if let Some(s) = sqlx::query_as::<_, Season>(
        "SELECT id, number, name, starts_at, ends_at, closed FROM seasons WHERE closed = 0
         ORDER BY number DESC LIMIT 1",
    )
    .fetch_optional(&mut *conn)
    .await?
    {
        return Ok(s);
    }
    open_next(conn, days, now()).await
}

async fn open_next(conn: &mut SqliteConnection, days: i64, start: i64) -> sqlx::Result<Season> {
    let last: Option<i64> = sqlx::query_scalar("SELECT MAX(number) FROM seasons")
        .fetch_one(&mut *conn)
        .await?;
    let number = last.unwrap_or(0) + 1;
    let ends = start + days.max(1) * 86_400;
    let id =
        sqlx::query("INSERT INTO seasons (number, name, starts_at, ends_at) VALUES (?, ?, ?, ?)")
            .bind(number)
            .bind(season_name(number))
            .bind(start)
            .bind(ends)
            .execute(&mut *conn)
            .await?
            .last_insert_rowid();
    tracing::info!(number, "nuova stagione aperta");
    Ok(Season {
        id,
        number,
        name: season_name(number),
        starts_at: start,
        ends_at: ends,
        closed: false,
    })
}

/// Punteggio stagionale dell'account (inizializzato col reset morbido).
pub async fn rating(
    conn: &mut SqliteConnection,
    season: &Season,
    account: i64,
) -> sqlx::Result<i64> {
    if let Some(r) = sqlx::query_scalar::<_, i64>(
        "SELECT rating FROM season_ratings WHERE season_id = ? AND account_id = ?",
    )
    .bind(season.id)
    .bind(account)
    .fetch_optional(&mut *conn)
    .await?
    {
        return Ok(r);
    }
    let prev: Option<i64> = sqlx::query_scalar(
        "SELECT sr.rating FROM season_ratings sr JOIN seasons s ON s.id = sr.season_id
         WHERE sr.account_id = ? AND s.number < ? ORDER BY s.number DESC LIMIT 1",
    )
    .bind(account)
    .bind(season.number)
    .fetch_optional(&mut *conn)
    .await?;
    let start = prev.map_or(BASE_RATING, |p| BASE_RATING + (p - BASE_RATING) / 2);
    sqlx::query(
        "INSERT INTO season_ratings (season_id, account_id, rating, peak) VALUES (?, ?, ?, ?)",
    )
    .bind(season.id)
    .bind(account)
    .bind(start)
    .bind(start)
    .execute(&mut *conn)
    .await?;
    Ok(start)
}

pub async fn record(
    conn: &mut SqliteConnection,
    season: &Season,
    account: i64,
    new_rating: i64,
    result: &str,
) -> sqlx::Result<()> {
    let (w, l, d) = match result {
        "win" => (1, 0, 0),
        "loss" => (0, 1, 0),
        _ => (0, 0, 1),
    };
    sqlx::query(
        "UPDATE season_ratings SET rating = ?, peak = MAX(peak, ?), wins = wins + ?, losses = losses + ?,
                draws = draws + ? WHERE season_id = ? AND account_id = ?",
    )
    .bind(new_rating)
    .bind(new_rating)
    .bind(w)
    .bind(l)
    .bind(d)
    .bind(season.id)
    .bind(account)
    .execute(conn)
    .await?;
    Ok(())
}

pub async fn add_clan_points(
    conn: &mut SqliteConnection,
    season: &Season,
    clan: i64,
    points: i64,
    win: bool,
) -> sqlx::Result<()> {
    if points == 0 {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO clan_season_points (season_id, clan_id, points, wins) VALUES (?, ?, ?, ?)
         ON CONFLICT(season_id, clan_id) DO UPDATE SET points = points + excluded.points,
             wins = wins + excluded.wins",
    )
    .bind(season.id)
    .bind(clan)
    .bind(points)
    .bind(win as i64)
    .execute(conn)
    .await?;
    Ok(())
}

/// Premi in monete per posizione finale.
pub fn coins_for_rank(rank: i64) -> i64 {
    match rank {
        1 => 1000,
        2 => 600,
        3 => 400,
        4..=10 => 200,
        _ => 50,
    }
}

fn titles_for_rank(number: i64, rank: i64) -> Vec<String> {
    let mut t = Vec::new();
    if rank == 1 {
        t.push(format!("Campione della Stagione {number}"));
    }
    if rank <= 3 {
        t.push(format!("Podio della Stagione {number}"));
    }
    if rank <= 10 {
        t.push(format!("Élite della Stagione {number}"));
    }
    t
}

async fn award(
    conn: &mut SqliteConnection,
    account: i64,
    title: &str,
    season: i64,
    coins: i64,
) -> sqlx::Result<()> {
    sqlx::query("INSERT OR IGNORE INTO titles (account_id, title, season_id, awarded_at) VALUES (?, ?, ?, ?)")
        .bind(account)
        .bind(title)
        .bind(season)
        .bind(now())
        .execute(&mut *conn)
        .await?;
    if coins > 0 {
        sqlx::query("UPDATE accounts SET coins = coins + ? WHERE id = ?")
            .bind(coins)
            .bind(account)
            .execute(conn)
            .await?;
    }
    Ok(())
}

/// Se la stagione attiva è scaduta (rispetto a `at`) la chiude, assegna premi
/// e apre la successiva. Restituisce la stagione chiusa.
pub async fn rollover_if_due(db: &SqlitePool, days: i64, at: i64) -> sqlx::Result<Option<Season>> {
    let mut tx = db.begin().await?;
    let season = current(&mut tx, days).await?;
    if at < season.ends_at {
        tx.rollback().await?;
        return Ok(None);
    }

    // Classifica giocatori: solo chi ha giocato almeno una partita.
    let players: Vec<(i64,)> = sqlx::query_as(
        "SELECT account_id FROM season_ratings
         WHERE season_id = ? AND wins + losses + draws > 0
         ORDER BY rating DESC, peak DESC, account_id",
    )
    .bind(season.id)
    .fetch_all(&mut *tx)
    .await?;
    for (i, (acc,)) in players.iter().enumerate() {
        let rank = i as i64 + 1;
        sqlx::query(
            "UPDATE season_ratings SET final_rank = ? WHERE season_id = ? AND account_id = ?",
        )
        .bind(rank)
        .bind(season.id)
        .bind(acc)
        .execute(&mut *tx)
        .await?;
        let titles = titles_for_rank(season.number, rank);
        let coins = coins_for_rank(rank);
        if titles.is_empty() {
            award_coins(&mut tx, *acc, coins).await?;
        }
        for (k, t) in titles.iter().enumerate() {
            award(&mut tx, *acc, t, season.id, if k == 0 { coins } else { 0 }).await?;
        }
    }

    // Classifica clan e premio ai membri del clan vincitore.
    let clans: Vec<(i64,)> = sqlx::query_as(
        "SELECT clan_id FROM clan_season_points WHERE season_id = ? AND points > 0
         ORDER BY points DESC, wins DESC, clan_id",
    )
    .bind(season.id)
    .fetch_all(&mut *tx)
    .await?;
    for (i, (clan,)) in clans.iter().enumerate() {
        sqlx::query(
            "UPDATE clan_season_points SET final_rank = ? WHERE season_id = ? AND clan_id = ?",
        )
        .bind(i as i64 + 1)
        .bind(season.id)
        .bind(clan)
        .execute(&mut *tx)
        .await?;
        if i == 0 {
            let members: Vec<(i64,)> =
                sqlx::query_as("SELECT account_id FROM clan_members WHERE clan_id = ?")
                    .bind(clan)
                    .fetch_all(&mut *tx)
                    .await?;
            for (acc,) in members {
                award(
                    &mut tx,
                    acc,
                    &format!("Clan campione della Stagione {}", season.number),
                    season.id,
                    300,
                )
                .await?;
            }
        }
    }

    sqlx::query("UPDATE seasons SET closed = 1 WHERE id = ?")
        .bind(season.id)
        .execute(&mut *tx)
        .await?;
    open_next(&mut tx, days, at).await?;
    tx.commit().await?;
    tracing::info!(
        number = season.number,
        players = players.len(),
        "stagione chiusa"
    );
    Ok(Some(season))
}

async fn award_coins(conn: &mut SqliteConnection, account: i64, coins: i64) -> sqlx::Result<()> {
    sqlx::query("UPDATE accounts SET coins = coins + ? WHERE id = ?")
        .bind(coins)
        .bind(account)
        .execute(conn)
        .await?;
    Ok(())
}

// ---------------------------------------------------------------- consultazione

#[derive(Serialize)]
pub struct Standing {
    pub rank: i64,
    pub username: String,
    pub rating: i64,
    pub peak: i64,
    pub wins: i64,
    pub losses: i64,
    pub draws: i64,
    pub clan: Option<String>,
}

#[derive(Serialize)]
pub struct ClanStanding {
    pub rank: i64,
    pub tag: String,
    pub name: String,
    pub points: i64,
    pub wins: i64,
}

#[derive(Serialize)]
pub struct SeasonView {
    #[serde(flatten)]
    pub season: Season,
    pub players: Vec<Standing>,
    pub clans: Vec<ClanStanding>,
}

pub async fn view(db: &SqlitePool, season: Season, limit: i64) -> sqlx::Result<SeasonView> {
    let players = sqlx::query_as::<_, (String, i64, i64, i64, i64, i64, Option<String>)>(
        "SELECT a.username, sr.rating, sr.peak, sr.wins, sr.losses, sr.draws, c.tag
         FROM season_ratings sr JOIN accounts a ON a.id = sr.account_id
         LEFT JOIN clan_members cm ON cm.account_id = a.id LEFT JOIN clans c ON c.id = cm.clan_id
         WHERE sr.season_id = ? AND sr.wins + sr.losses + sr.draws > 0
         ORDER BY COALESCE(sr.final_rank, 1000000), sr.rating DESC, sr.peak DESC, a.id LIMIT ?",
    )
    .bind(season.id)
    .bind(limit)
    .fetch_all(db)
    .await?
    .into_iter()
    .enumerate()
    .map(
        |(i, (username, rating, peak, wins, losses, draws, clan))| Standing {
            rank: i as i64 + 1,
            username,
            rating,
            peak,
            wins,
            losses,
            draws,
            clan,
        },
    )
    .collect();
    let clans = sqlx::query_as::<_, (String, String, i64, i64)>(
        "SELECT c.tag, c.name, p.points, p.wins FROM clan_season_points p JOIN clans c ON c.id = p.clan_id
         WHERE p.season_id = ? ORDER BY COALESCE(p.final_rank, 1000000), p.points DESC, p.wins DESC, c.id LIMIT ?",
    )
    .bind(season.id)
    .bind(limit)
    .fetch_all(db)
    .await?
    .into_iter()
    .enumerate()
    .map(|(i, (tag, name, points, wins))| ClanStanding { rank: i as i64 + 1, tag, name, points, wins })
    .collect();
    Ok(SeasonView {
        season,
        players,
        clans,
    })
}

pub async fn by_number(db: &SqlitePool, number: i64) -> sqlx::Result<Option<Season>> {
    sqlx::query_as(
        "SELECT id, number, name, starts_at, ends_at, closed FROM seasons WHERE number = ?",
    )
    .bind(number)
    .fetch_optional(db)
    .await
}

pub async fn all(db: &SqlitePool) -> sqlx::Result<Vec<Season>> {
    sqlx::query_as(
        "SELECT id, number, name, starts_at, ends_at, closed FROM seasons ORDER BY number DESC",
    )
    .fetch_all(db)
    .await
}

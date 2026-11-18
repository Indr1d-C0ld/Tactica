//! Statistiche avanzate: storico delle proprie partite, riepilogo di una
//! partita unità per unità, andamento personale e statistiche globali estese.
#![allow(clippy::type_complexity)]

use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::AppState;
use crate::auth::AuthUser;
use crate::db::now;
use crate::error::{ApiError, ApiResult};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/me/matches", get(my_matches))
        .route("/api/me/stats", get(my_stats))
        .route("/api/matches/{id}/summary", get(summary))
        .route("/api/stats/trends", get(trends))
}

// ------------------------------------------------------------------ storico

#[derive(Deserialize)]
struct HistoryQuery {
    #[serde(default)]
    mode: String,
    #[serde(default)]
    offset: i64,
    #[serde(default)]
    limit: Option<i64>,
}

#[derive(Serialize)]
struct MyUnit {
    name: Option<String>,
    class: String,
    level_start: i64,
    level_end: i64,
    survived: bool,
    damage_dealt: i64,
    healing_done: i64,
    kills: i64,
    xp_gained: i64,
}

#[derive(Serialize)]
struct HistoryRow {
    id: i64,
    mode: String,
    result: String,
    biome: String,
    turns: i64,
    elo_before: i64,
    elo_after: i64,
    coins: i64,
    ended_at: i64,
    /// Altri giocatori umani: alleati e avversari (vuoto = contro l'IA).
    allies: Vec<String>,
    opponents: Vec<String>,
    units: Vec<MyUnit>,
}

#[derive(Serialize)]
struct HistoryPage {
    total: i64,
    matches: Vec<HistoryRow>,
}

async fn my_matches(
    State(s): State<AppState>,
    user: AuthUser,
    Query(q): Query<HistoryQuery>,
) -> ApiResult<Json<HistoryPage>> {
    let limit = q.limit.unwrap_or(20).clamp(1, 100);
    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM match_players p JOIN matches m ON m.id = p.match_id
         WHERE p.account_id = ?1 AND (?2 = '' OR m.mode = ?2)",
    )
    .bind(user.id)
    .bind(&q.mode)
    .fetch_one(&s.db)
    .await?;
    let rows: Vec<(i64, String, String, String, i64, i64, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT m.id, m.mode, p.result, m.biome, m.turns, p.elo_before, p.elo_after, p.coins, m.ended_at, p.team
         FROM match_players p JOIN matches m ON m.id = p.match_id
         WHERE p.account_id = ?1 AND (?2 = '' OR m.mode = ?2)
         ORDER BY m.id DESC LIMIT ?3 OFFSET ?4",
    )
    .bind(user.id)
    .bind(&q.mode)
    .bind(limit)
    .bind(q.offset.max(0))
    .fetch_all(&s.db)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for (id, mode, result, biome, turns, elo_before, elo_after, coins, ended_at, team) in rows {
        let others: Vec<(String, i64)> = sqlx::query_as(
            "SELECT a.username, p.team FROM match_players p JOIN accounts a ON a.id = p.account_id
             WHERE p.match_id = ? AND p.account_id != ?",
        )
        .bind(id)
        .bind(user.id)
        .fetch_all(&s.db)
        .await?;
        let units: Vec<(Option<String>, String, i64, i64, bool, i64, i64, i64, i64)> =
            sqlx::query_as(
                "SELECT r.name, u.class, u.level_start, u.level_end, u.survived, u.damage_dealt,
                    u.healing_done, u.kills, u.xp_gained
             FROM match_units u LEFT JOIN roster r ON r.id = u.roster_id
             WHERE u.match_id = ? AND u.account_id = ? ORDER BY u.unit",
            )
            .bind(id)
            .bind(user.id)
            .fetch_all(&s.db)
            .await?;
        out.push(HistoryRow {
            id,
            mode,
            result,
            biome,
            turns,
            elo_before,
            elo_after,
            coins,
            ended_at,
            allies: others
                .iter()
                .filter(|o| o.1 == team)
                .map(|o| o.0.clone())
                .collect(),
            opponents: others
                .iter()
                .filter(|o| o.1 != team)
                .map(|o| o.0.clone())
                .collect(),
            units: units
                .into_iter()
                .map(|u| MyUnit {
                    name: u.0,
                    class: u.1,
                    level_start: u.2,
                    level_end: u.3,
                    survived: u.4,
                    damage_dealt: u.5,
                    healing_done: u.6,
                    kills: u.7,
                    xp_gained: u.8,
                })
                .collect(),
        });
    }
    Ok(Json(HistoryPage {
        total,
        matches: out,
    }))
}

// ------------------------------------------------------------------ riepilogo di una partita

#[derive(Serialize)]
struct SummaryUnit {
    team: i64,
    owner: Option<String>,
    name: Option<String>,
    class: String,
    level_start: i64,
    level_end: i64,
    survived: bool,
    damage_dealt: i64,
    damage_taken: i64,
    healing_done: i64,
    kills: i64,
    actions: i64,
    misses: i64,
    violations: i64,
    xp_gained: i64,
}

#[derive(Serialize)]
struct SummaryPlayer {
    username: String,
    team: i64,
    result: String,
    elo_before: i64,
    elo_after: i64,
    coins: i64,
}

#[derive(Serialize)]
struct Summary {
    id: i64,
    mode: String,
    biome: String,
    turns: i64,
    winner_team: Option<i64>,
    started_at: i64,
    ended_at: i64,
    statutes: Vec<String>,
    players: Vec<SummaryPlayer>,
    units: Vec<SummaryUnit>,
}

async fn summary(
    State(s): State<AppState>,
    _user: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Summary>> {
    let head: (i64, String, String, i64, Option<i64>, i64, i64) = sqlx::query_as(
        "SELECT id, mode, biome, turns, winner_team, started_at, ended_at FROM matches WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&s.db)
    .await?
    .ok_or_else(|| ApiError::not_found("partita inesistente"))?;
    let statutes: Vec<String> =
        sqlx::query_scalar("SELECT statute FROM match_statutes WHERE match_id = ?")
            .bind(id)
            .fetch_all(&s.db)
            .await?;
    let players: Vec<(String, i64, String, i64, i64, i64)> = sqlx::query_as(
        "SELECT a.username, p.team, p.result, p.elo_before, p.elo_after, p.coins
         FROM match_players p JOIN accounts a ON a.id = p.account_id WHERE p.match_id = ? ORDER BY p.team",
    )
    .bind(id)
    .fetch_all(&s.db)
    .await?;
    type U = (
        i64,
        Option<String>,
        Option<String>,
        String,
        i64,
        i64,
        bool,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
    );
    let units: Vec<U> = sqlx::query_as(
        "SELECT u.team, a.username, r.name, u.class, u.level_start, u.level_end, u.survived,
                u.damage_dealt, u.damage_taken, u.healing_done, u.kills, u.actions, u.misses,
                u.violations, u.xp_gained
         FROM match_units u LEFT JOIN accounts a ON a.id = u.account_id LEFT JOIN roster r ON r.id = u.roster_id
         WHERE u.match_id = ? ORDER BY u.team, u.unit",
    )
    .bind(id)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(Summary {
        id: head.0,
        mode: head.1,
        biome: head.2,
        turns: head.3,
        winner_team: head.4,
        started_at: head.5,
        ended_at: head.6,
        statutes,
        players: players
            .into_iter()
            .map(|p| SummaryPlayer {
                username: p.0,
                team: p.1,
                result: p.2,
                elo_before: p.3,
                elo_after: p.4,
                coins: p.5,
            })
            .collect(),
        units: units
            .into_iter()
            .map(|u| SummaryUnit {
                team: u.0,
                owner: u.1,
                name: u.2,
                class: u.3,
                level_start: u.4,
                level_end: u.5,
                survived: u.6,
                damage_dealt: u.7,
                damage_taken: u.8,
                healing_done: u.9,
                kills: u.10,
                actions: u.11,
                misses: u.12,
                violations: u.13,
                xp_gained: u.14,
            })
            .collect(),
    }))
}

// ------------------------------------------------------------------ statistiche personali

#[derive(Serialize)]
struct EloPoint {
    at: i64,
    elo: i64,
}

#[derive(Serialize)]
struct ModeLine {
    mode: String,
    played: i64,
    wins: i64,
    losses: i64,
    draws: i64,
    avg_turns: f64,
    coins: i64,
}

#[derive(Serialize)]
struct BiomeLine {
    biome: String,
    played: i64,
    wins: i64,
}

#[derive(Serialize)]
struct UnitLine {
    name: String,
    class: String,
    people: String,
    level: i64,
    fielded: i64,
    kills: i64,
    damage_dealt: i64,
    healing_done: i64,
    survival_rate: f64,
    xp_gained: i64,
}

#[derive(Serialize)]
struct MyStats {
    played: i64,
    wins: i64,
    losses: i64,
    draws: i64,
    best_elo: i64,
    current_streak: i64,
    best_streak: i64,
    coins_earned: i64,
    accuracy: f64,
    elo: Vec<EloPoint>,
    modes: Vec<ModeLine>,
    biomes: Vec<BiomeLine>,
    units: Vec<UnitLine>,
}

async fn my_stats(State(s): State<AppState>, user: AuthUser) -> ApiResult<Json<MyStats>> {
    Ok(Json(personal(&s.db, user.id).await?))
}

async fn personal(db: &SqlitePool, id: i64) -> sqlx::Result<MyStats> {
    let results: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT p.result, p.elo_after, m.ended_at FROM match_players p JOIN matches m ON m.id = p.match_id
         WHERE p.account_id = ? ORDER BY m.id",
    )
    .bind(id)
    .fetch_all(db)
    .await?;
    let (mut wins, mut losses, mut draws, mut cur, mut best) = (0, 0, 0, 0i64, 0i64);
    let mut best_elo = 1000;
    for (r, elo, _) in &results {
        match r.as_str() {
            "win" => {
                wins += 1;
                cur = cur.max(0) + 1;
            }
            "loss" => {
                losses += 1;
                cur = cur.min(0) - 1;
            }
            _ => {
                draws += 1;
                cur = 0;
            }
        }
        best = best.max(cur);
        best_elo = best_elo.max(*elo);
    }
    // Andamento ELO: al più 120 punti, gli ultimi.
    let skip = results.len().saturating_sub(120);
    let elo = results
        .iter()
        .skip(skip)
        .map(|(_, e, at)| EloPoint { at: *at, elo: *e })
        .collect();
    let coins_earned: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(coins), 0) FROM match_players WHERE account_id = ?",
    )
    .bind(id)
    .fetch_one(db)
    .await?;
    let accuracy: f64 = sqlx::query_scalar(
        "SELECT COALESCE(1.0 - CAST(SUM(misses) AS REAL) / NULLIF(SUM(actions), 0), 0.0) FROM match_units WHERE account_id = ?",
    )
    .bind(id)
    .fetch_one(db)
    .await?;
    let modes = sqlx::query_as::<_, (String, i64, i64, i64, i64, f64, i64)>(
        "SELECT m.mode, COUNT(*), SUM(p.result = 'win'), SUM(p.result = 'loss'), SUM(p.result = 'draw'),
                COALESCE(AVG(m.turns), 0.0), SUM(p.coins)
         FROM match_players p JOIN matches m ON m.id = p.match_id
         WHERE p.account_id = ? GROUP BY m.mode ORDER BY COUNT(*) DESC",
    )
    .bind(id)
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| ModeLine {
        mode: r.0,
        played: r.1,
        wins: r.2,
        losses: r.3,
        draws: r.4,
        avg_turns: r.5,
        coins: r.6,
    })
    .collect();
    let biomes = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT m.biome, COUNT(*), SUM(p.result = 'win')
         FROM match_players p JOIN matches m ON m.id = p.match_id
         WHERE p.account_id = ? GROUP BY m.biome ORDER BY COUNT(*) DESC",
    )
    .bind(id)
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| BiomeLine {
        biome: r.0,
        played: r.1,
        wins: r.2,
    })
    .collect();
    let units = sqlx::query_as::<_, (String, String, String, i64, i64, i64, i64, i64, f64, i64)>(
        "SELECT r.name, r.class, r.people, r.level, COUNT(u.match_id), COALESCE(SUM(u.kills), 0),
                COALESCE(SUM(u.damage_dealt), 0), COALESCE(SUM(u.healing_done), 0),
                COALESCE(AVG(u.survived), 0.0), COALESCE(SUM(u.xp_gained), 0)
         FROM roster r LEFT JOIN match_units u ON u.roster_id = r.id
         WHERE r.account_id = ? GROUP BY r.id ORDER BY COUNT(u.match_id) DESC, r.id",
    )
    .bind(id)
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| UnitLine {
        name: r.0,
        class: r.1,
        people: r.2,
        level: r.3,
        fielded: r.4,
        kills: r.5,
        damage_dealt: r.6,
        healing_done: r.7,
        survival_rate: r.8,
        xp_gained: r.9,
    })
    .collect();
    Ok(MyStats {
        played: results.len() as i64,
        wins,
        losses,
        draws,
        best_elo,
        current_streak: cur,
        best_streak: best,
        coins_earned,
        accuracy,
        elo,
        modes,
        biomes,
        units,
    })
}

// ------------------------------------------------------------------ andamento globale

#[derive(Serialize)]
struct Day {
    day: i64,
    matches: i64,
    new_accounts: i64,
    players: i64,
}

#[derive(Serialize)]
struct Share {
    key: String,
    count: i64,
    win_rate: f64,
}

#[derive(Serialize)]
struct Trends {
    days: Vec<Day>,
    modes: Vec<Share>,
    peoples: Vec<Share>,
    avg_turns: f64,
    avg_match_minutes: f64,
}

/// Ultimi 30 giorni, modalità, popoli (dalle unità schierate), durata media.
async fn trends(State(s): State<AppState>) -> ApiResult<Json<Trends>> {
    let db = &s.db;
    let today = now().div_euclid(86_400);
    let mut days = Vec::with_capacity(30);
    for d in (today - 29)..=today {
        let (a, b) = (d * 86_400, (d + 1) * 86_400);
        let (matches, players): (i64, i64) = sqlx::query_as(
            "SELECT COUNT(DISTINCT m.id), COUNT(DISTINCT p.account_id)
             FROM matches m LEFT JOIN match_players p ON p.match_id = m.id
             WHERE m.ended_at >= ? AND m.ended_at < ?",
        )
        .bind(a)
        .bind(b)
        .fetch_one(db)
        .await?;
        let new_accounts: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM accounts WHERE created_at >= ? AND created_at < ?",
        )
        .bind(a)
        .bind(b)
        .fetch_one(db)
        .await?;
        days.push(Day {
            day: d * 86_400,
            matches,
            new_accounts,
            players,
        });
    }
    let modes = sqlx::query_as::<_, (String, i64, f64)>(
        "SELECT m.mode, COUNT(DISTINCT m.id), COALESCE(AVG(p.result = 'win'), 0.0)
         FROM matches m LEFT JOIN match_players p ON p.match_id = m.id GROUP BY m.mode ORDER BY 2 DESC",
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| Share {
        key: r.0,
        count: r.1,
        win_rate: r.2,
    })
    .collect();
    let peoples = sqlx::query_as::<_, (String, i64, f64)>(
        "SELECT r.people, COUNT(*), COALESCE(AVG(m.winner_team IS NOT NULL AND m.winner_team = u.team), 0.0)
         FROM match_units u JOIN roster r ON r.id = u.roster_id JOIN matches m ON m.id = u.match_id
         GROUP BY r.people ORDER BY 2 DESC",
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| Share {
        key: r.0,
        count: r.1,
        win_rate: r.2,
    })
    .collect();
    let (avg_turns, avg_secs): (f64, f64) = sqlx::query_as(
        "SELECT COALESCE(AVG(turns), 0.0), COALESCE(AVG(ended_at - started_at), 0.0) FROM matches",
    )
    .fetch_one(db)
    .await?;
    Ok(Json(Trends {
        days,
        modes,
        peoples,
        avg_turns,
        avg_match_minutes: avg_secs / 60.0,
    }))
}

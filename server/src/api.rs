//! Rotte HTTP: account, roster, profili, classifiche, statistiche, replay.

use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tactica_core::class::Class;

use crate::AppState;
use crate::auth::{self, AuthUser};
use crate::db::{self, MAX_ROSTER, RosterUnit, now};
use crate::error::{ApiError, ApiResult};
use crate::stats;
use crate::ws;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/api/register", post(auth::register))
        .route("/api/login", post(auth::login))
        .route("/api/logout", post(auth::logout))
        .route("/api/me", get(me))
        .route("/api/classes", get(classes))
        .route("/api/roster", get(roster).post(recruit))
        .route("/api/roster/{id}", delete(dismiss))
        .route("/api/players/{name}", get(player))
        .route("/api/leaderboard", get(leaderboard))
        .route("/api/stats", get(global))
        .route("/api/matches/{id}", get(match_detail))
        .route("/ws", get(ws::upgrade))
}

#[derive(Serialize)]
struct Me {
    id: i64,
    username: String,
    elo: i64,
    roster: Vec<RosterUnit>,
}

async fn me(State(s): State<AppState>, user: AuthUser) -> ApiResult<Json<Me>> {
    let elo = sqlx::query_scalar("SELECT elo FROM accounts WHERE id = ?")
        .bind(user.id)
        .fetch_one(&s.db)
        .await?;
    Ok(Json(Me {
        id: user.id,
        username: user.username,
        elo,
        roster: db::roster(&s.db, user.id).await?,
    }))
}

#[derive(Serialize)]
struct ClassInfo {
    key: &'static str,
    name: &'static str,
    abilities: Vec<&'static str>,
}

async fn classes() -> Json<Vec<ClassInfo>> {
    Json(
        Class::ALL
            .iter()
            .map(|c| ClassInfo {
                key: c.key(),
                name: c.name(),
                abilities: c.abilities().iter().map(|a| a.def().name).collect(),
            })
            .collect(),
    )
}

async fn roster(State(s): State<AppState>, user: AuthUser) -> ApiResult<Json<Vec<RosterUnit>>> {
    Ok(Json(db::roster(&s.db, user.id).await?))
}

#[derive(Deserialize)]
struct Recruit {
    name: String,
    class: String,
}

async fn recruit(
    State(s): State<AppState>,
    user: AuthUser,
    Json(r): Json<Recruit>,
) -> ApiResult<Json<RosterUnit>> {
    let name = r.name.trim().to_string();
    if name.is_empty() || name.chars().count() > 16 || name.chars().any(char::is_control) {
        return Err(ApiError::bad_request("nome: da 1 a 16 caratteri"));
    }
    let class =
        Class::from_key(&r.class).ok_or_else(|| ApiError::bad_request("classe sconosciuta"))?;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM roster WHERE account_id = ?")
        .bind(user.id)
        .fetch_one(&s.db)
        .await?;
    if count >= MAX_ROSTER {
        return Err(ApiError::conflict(format!(
            "massimo {MAX_ROSTER} unità nel roster"
        )));
    }
    let id =
        sqlx::query("INSERT INTO roster (account_id, name, class, created_at) VALUES (?, ?, ?, ?)")
            .bind(user.id)
            .bind(&name)
            .bind(class.key())
            .bind(now())
            .execute(&s.db)
            .await?
            .last_insert_rowid();
    Ok(Json(RosterUnit {
        id,
        name,
        class: class.key().to_string(),
        level: 1,
        xp: 0,
    }))
}

async fn dismiss(State(s): State<AppState>, user: AuthUser, Path(id): Path<i64>) -> ApiResult<()> {
    if s.busy.lock().unwrap().contains(&user.id) {
        return Err(ApiError::conflict("non durante una partita"));
    }
    let n = sqlx::query("DELETE FROM roster WHERE id = ? AND account_id = ?")
        .bind(id)
        .bind(user.id)
        .execute(&s.db)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(ApiError::not_found("unità non trovata"));
    }
    Ok(())
}

async fn player(
    State(s): State<AppState>,
    Path(name): Path<String>,
) -> ApiResult<Json<stats::Profile>> {
    stats::profile(&s.db, &name)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("giocatore non trovato"))
}

#[derive(Deserialize)]
struct Limit {
    limit: Option<i64>,
}

async fn leaderboard(
    State(s): State<AppState>,
    Query(q): Query<Limit>,
) -> ApiResult<Json<Vec<stats::LeaderRow>>> {
    let limit = q.limit.unwrap_or(20).clamp(1, 100);
    Ok(Json(stats::leaderboard(&s.db, limit).await?))
}

async fn global(State(s): State<AppState>) -> ApiResult<Json<stats::Global>> {
    Ok(Json(stats::global(&s.db).await?))
}

async fn match_detail(
    State(s): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<stats::MatchDetail>> {
    stats::match_detail(&s.db, id)
        .await
        .map_err(ApiError::internal)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("partita non trovata"))
}

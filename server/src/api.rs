//! Rotte HTTP: account, roster, profili, classifiche, statistiche, replay.

use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tactica_core::class::Class;
use tactica_core::item::starter_kit;

use crate::AppState;
use crate::armory;
use crate::auth::{self, AuthUser};
use crate::db::{self, MAX_ROSTER, RosterUnit, items_json, now};
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
        .route("/api/classes", get(armory::classes))
        .route("/api/items", get(armory::items))
        .route("/api/inventory", get(armory::inventory))
        .route("/api/shop/buy", post(armory::buy))
        .route("/api/shop/sell", post(armory::sell))
        .route("/api/roster", get(roster).post(recruit))
        .route("/api/roster/{id}", delete(dismiss))
        .route("/api/roster/{id}/equipment", put(armory::equip))
        .route("/api/roster/{id}/class", put(armory::change_class))
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
    coins: i64,
    roster: Vec<RosterUnit>,
}

async fn me(State(s): State<AppState>, user: AuthUser) -> ApiResult<Json<Me>> {
    let (elo, coins): (i64, i64) = sqlx::query_as("SELECT elo, coins FROM accounts WHERE id = ?")
        .bind(user.id)
        .fetch_one(&s.db)
        .await?;
    Ok(Json(Me {
        id: user.id,
        username: user.username,
        elo,
        coins,
        roster: db::roster(&s.db, user.id).await?,
    }))
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
    if !class.is_base() {
        return Err(ApiError::bad_request(
            "le reclute partono da una classe base; le avanzate si sbloccano con la padronanza",
        ));
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM roster WHERE account_id = ?")
        .bind(user.id)
        .fetch_one(&s.db)
        .await?;
    if count >= MAX_ROSTER {
        return Err(ApiError::conflict(format!(
            "massimo {MAX_ROSTER} unità nel roster"
        )));
    }
    let id = sqlx::query(
        "INSERT INTO roster (account_id, name, class, equipment, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(user.id)
    .bind(&name)
    .bind(class.key())
    .bind(items_json(&starter_kit(class)))
    .bind(now())
    .execute(&s.db)
    .await?
    .last_insert_rowid();
    db::roster_unit(&s.db, user.id, id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::internal("recluta appena creata non trovata"))
}

async fn dismiss(State(s): State<AppState>, user: AuthUser, Path(id): Path<i64>) -> ApiResult<()> {
    if s.busy.lock().unwrap().contains(&user.id) {
        return Err(ApiError::conflict("non durante una partita"));
    }
    let unit = db::roster_unit(&s.db, user.id, id)
        .await?
        .ok_or_else(|| ApiError::not_found("unità non trovata"))?;
    // L'equipaggiamento torna nell'inventario.
    let mut tx = s.db.begin().await?;
    for item in unit.items() {
        db::add_item(&mut tx, user.id, item, 1).await?;
    }
    sqlx::query("DELETE FROM roster WHERE id = ? AND account_id = ?")
        .bind(id)
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
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

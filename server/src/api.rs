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
use crate::{clans, seasons, tournaments, wars};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/api/version", get(crate::version::info))
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
        .route("/api/roster/{id}/bag", put(armory::set_bag))
        .route("/api/craft", get(crate::craft::view))
        .route("/api/craft/salvage", post(crate::craft::salvage))
        .route("/api/craft/refine", post(crate::craft::refine))
        .route("/api/craft/enchant", post(crate::craft::enchant))
        .route("/api/craft/brew", post(crate::craft::brew))
        .route("/api/players/{name}", get(player))
        .route("/api/leaderboard", get(leaderboard))
        .route("/api/stats", get(global))
        .route("/api/matches/{id}", get(match_detail))
        .route("/api/clans", get(clans::list).post(clans::create))
        .route("/api/clans/leave", post(clans::leave))
        .route("/api/clans/{tag}", get(clans::detail).put(clans::update))
        .route("/api/clans/{tag}/join", post(clans::join))
        .route("/api/clans/{tag}/invite", post(clans::invite))
        .route("/api/clans/{tag}/kick", post(clans::kick))
        .route("/api/clans/{tag}/role", post(clans::set_role))
        .route("/api/wars", get(wars::list).post(wars::declare))
        .route("/api/wars/{id}/accept", post(wars::accept))
        .route("/api/wars/{id}/decline", post(wars::decline))
        .route("/api/wars/{id}/cancel", post(wars::cancel))
        .route(
            "/api/tournaments",
            get(tournaments::list).post(tournaments::create),
        )
        .route("/api/tournaments/{id}", get(tournaments::detail))
        .route("/api/tournaments/{id}/join", post(tournaments::join))
        .route("/api/tournaments/{id}/leave", post(tournaments::leave))
        .route("/api/tournaments/{id}/start", post(tournaments::start))
        .route("/api/tournaments/{id}/cancel", post(tournaments::cancel))
        .route("/api/campaign", get(crate::campaign::view))
        .route("/api/seasons", get(season_list))
        .route("/api/seasons/current", get(season_current))
        .route("/api/seasons/{number}", get(season_by_number))
        .route("/ws", get(ws::upgrade))
        .merge(crate::admin::routes())
        .merge(crate::account::routes())
        .merge(crate::achievements::routes())
        .merge(crate::history::routes())
}

#[derive(Serialize)]
struct Me {
    id: i64,
    username: String,
    elo: i64,
    coins: i64,
    roster: Vec<RosterUnit>,
    clan: Option<clans::Membership>,
    invites: Vec<String>,
    titles: Vec<String>,
    season: MySeason,
    is_admin: bool,
    /// Messaggio per tutti i giocatori (vuoto = nessuno).
    motd: String,
    maintenance: bool,
    /// Traguardi sbloccati e non ancora mostrati (POST /api/me/achievements/seen).
    new_achievements: Vec<crate::achievements::Unlocked>,
    achievements: i64,
}

#[derive(Serialize)]
struct MySeason {
    number: i64,
    name: String,
    ends_at: i64,
    rating: Option<i64>,
    wins: i64,
    losses: i64,
}

async fn me(State(s): State<AppState>, user: AuthUser) -> ApiResult<Json<Me>> {
    // Prima i traguardi: possono dare monete.
    let (new_achievements, achievements) = {
        let mut tx = s.db.begin().await?;
        crate::achievements::check(&mut tx, user.id).await?;
        let new = crate::achievements::unseen(&mut tx, user.id).await?;
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM achievements WHERE account_id = ?")
            .bind(user.id)
            .fetch_one(&mut *tx)
            .await?;
        tx.commit().await?;
        (new, n)
    };
    let (elo, coins): (i64, i64) = sqlx::query_as("SELECT elo, coins FROM accounts WHERE id = ?")
        .bind(user.id)
        .fetch_one(&s.db)
        .await?;
    let mut conn = s.db.acquire().await?;
    let clan = clans::membership(&mut conn, user.id).await?;
    let season = seasons::current(&mut conn, s.config.season_days).await?;
    let mine: Option<(i64, i64, i64)> = sqlx::query_as(
        "SELECT rating, wins, losses FROM season_ratings WHERE season_id = ? AND account_id = ?",
    )
    .bind(season.id)
    .bind(user.id)
    .fetch_optional(&mut *conn)
    .await?;
    drop(conn);
    let (motd, maintenance) = {
        let st = s.settings.read().unwrap();
        (st.motd.clone(), st.maintenance)
    };
    Ok(Json(Me {
        id: user.id,
        clan,
        invites: clans::invites(&s.db, user.id).await?,
        titles: stats::titles(&s.db, user.id).await?,
        season: MySeason {
            number: season.number,
            name: season.name,
            ends_at: season.ends_at,
            rating: mine.map(|m| m.0),
            wins: mine.map_or(0, |m| m.1),
            losses: mine.map_or(0, |m| m.2),
        },
        is_admin: user.is_admin,
        motd,
        maintenance,
        new_achievements,
        achievements,
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
    /// Popolo (predefinito: umano).
    #[serde(default)]
    people: Option<String>,
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
    let people = match r.people.as_deref() {
        None => tactica_core::people::People::Human,
        Some(k) => tactica_core::people::People::from_key(k)
            .ok_or_else(|| ApiError::bad_request("popolo sconosciuto"))?,
    };
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
        "INSERT INTO roster (account_id, name, class, equipment, created_at, people) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(user.id)
    .bind(&name)
    .bind(class.key())
    .bind(items_json(&starter_kit(class).map(tactica_core::gear::ItemRef::from)))
    .bind(now())
    .bind(people.key())
    .execute(&s.db)
    .await?
    .last_insert_rowid();
    crate::achievements::record(&s.db, user.id, &[("recruits", 1)]).await;
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
    for item in unit.items().into_iter().chain(unit.bag_items()) {
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

async fn season_current(State(s): State<AppState>) -> ApiResult<Json<seasons::SeasonView>> {
    let mut conn = s.db.acquire().await?;
    let season = seasons::current(&mut conn, s.config.season_days).await?;
    drop(conn);
    Ok(Json(seasons::view(&s.db, season, 50).await?))
}

async fn season_by_number(
    State(s): State<AppState>,
    Path(number): Path<i64>,
) -> ApiResult<Json<seasons::SeasonView>> {
    let season = seasons::by_number(&s.db, number)
        .await?
        .ok_or_else(|| ApiError::not_found("stagione non trovata"))?;
    Ok(Json(seasons::view(&s.db, season, 100).await?))
}

async fn season_list(State(s): State<AppState>) -> ApiResult<Json<Vec<seasons::Season>>> {
    let mut conn = s.db.acquire().await?;
    seasons::current(&mut conn, s.config.season_days).await?;
    drop(conn);
    Ok(Json(seasons::all(&s.db).await?))
}

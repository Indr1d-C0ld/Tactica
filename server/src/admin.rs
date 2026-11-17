//! Pannello di amministrazione: panoramica, utenti (sospensioni, monete,
//! password, ruolo), connessioni aperte, partite, impostazioni di gioco,
//! stagioni, backup e registro delle azioni. Tutte le rotte richiedono un
//! account con `is_admin`; ogni modifica finisce in `admin_log`.
#![allow(clippy::type_complexity)]

use axum::extract::{FromRequestParts, Path, Query, State};
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::AppState;
use crate::auth::{self, AuthUser};
use crate::db::now;
use crate::error::{ApiError, ApiResult};
use crate::lobby::ToConn;
use crate::settings::Settings;

/// Fine di una sospensione a tempo indeterminato (31/12/9999).
pub const PERMANENT: i64 = 253_402_300_799;

/// Data leggibile (UTC) da secondi Unix.
pub fn date(t: i64) -> String {
    let days = t.div_euclid(86_400);
    let secs = t.rem_euclid(86_400);
    // Algoritmo di H. Hinnant: giorni dal 1970 -> data civile.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{d:02}/{m:02}/{y} {:02}:{:02} UTC",
        secs / 3600,
        secs % 3600 / 60
    )
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/admin/overview", get(overview))
        .route("/api/admin/users", get(users))
        .route("/api/admin/users/{id}", get(user_detail))
        .route("/api/admin/users/{id}/ban", post(ban))
        .route("/api/admin/users/{id}/unban", post(unban))
        .route("/api/admin/users/{id}/coins", post(coins))
        .route("/api/admin/users/{id}/password", post(password))
        .route("/api/admin/users/{id}/logout", post(logout))
        .route("/api/admin/users/{id}/role", post(role))
        .route("/api/admin/online", get(online))
        .route("/api/admin/online/{conn}/kick", post(kick))
        .route("/api/admin/matches", get(matches))
        .route("/api/admin/settings", get(get_settings).put(put_settings))
        .route("/api/admin/seasons/close", post(close_season))
        .route("/api/admin/backup", post(backup))
        .route("/api/admin/log", get(log_list))
}

/// Utente autenticato con ruolo di amministratore.
pub struct AdminUser(pub AuthUser);

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let user = AuthUser::from_request_parts(parts, state).await?;
        if !user.is_admin {
            return Err(ApiError::new(
                StatusCode::FORBIDDEN,
                "riservato agli amministratori",
            ));
        }
        Ok(AdminUser(user))
    }
}

async fn record(
    db: &SqlitePool,
    admin: &AuthUser,
    action: &str,
    target: Option<&str>,
    detail: &str,
) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO admin_log (at, admin_id, admin, action, target, detail) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(now())
        .bind(admin.id)
        .bind(&admin.username)
        .bind(action)
        .bind(target)
        .bind(detail)
        .execute(db)
        .await?;
    tracing::info!(admin = %admin.username, action, target, detail, "azione admin");
    Ok(())
}

async fn username_of(db: &SqlitePool, id: i64) -> ApiResult<String> {
    sqlx::query_scalar("SELECT username FROM accounts WHERE id = ?")
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or_else(|| ApiError::not_found("utente inesistente"))
}

/// Chiude le connessioni WebSocket di un account.
fn kick_account(state: &AppState, account: i64) -> usize {
    let online = state.online.lock().unwrap();
    let mut n = 0;
    for o in online.values().filter(|o| o.account_id == account) {
        let _ = o.tx.send(ToConn::Kick);
        n += 1;
    }
    n
}

// ------------------------------------------------------------------ panoramica

#[derive(Serialize)]
struct Overview {
    accounts: i64,
    admins: i64,
    banned: i64,
    new_24h: i64,
    new_7d: i64,
    active_24h: i64,
    active_7d: i64,
    matches: i64,
    matches_24h: i64,
    matches_7d: i64,
    online_connections: usize,
    online_players: usize,
    in_match: usize,
    pvp_queue: usize,
    coins_total: i64,
    items_total: i64,
    clans: i64,
    season: String,
    season_ends_at: i64,
    backups: bool,
    settings: Settings,
}

async fn overview(State(s): State<AppState>, _a: AdminUser) -> ApiResult<Json<Overview>> {
    let t = now();
    let (accounts, admins, banned, new_24h, new_7d, active_24h, active_7d, coins_total): (i64, i64, i64, i64, i64, i64, i64, i64) =
        sqlx::query_as(
            "SELECT COUNT(*), COALESCE(SUM(is_admin), 0),
                    COALESCE(SUM(banned_until IS NOT NULL AND banned_until > ?1), 0),
                    COALESCE(SUM(created_at > ?1 - 86400), 0), COALESCE(SUM(created_at > ?1 - 604800), 0),
                    COALESCE(SUM(last_seen > ?1 - 86400), 0), COALESCE(SUM(last_seen > ?1 - 604800), 0),
                    COALESCE(SUM(coins), 0)
             FROM accounts",
        )
        .bind(t)
        .fetch_one(&s.db)
        .await?;
    let (matches, matches_24h, matches_7d): (i64, i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(ended_at > ?1 - 86400), 0), COALESCE(SUM(ended_at > ?1 - 604800), 0) FROM matches",
    )
    .bind(t)
    .fetch_one(&s.db)
    .await?;
    let items_total: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(qty), 0) FROM inventory")
        .fetch_one(&s.db)
        .await?;
    let clans: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM clans")
        .fetch_one(&s.db)
        .await?;
    let mut conn = s.db.acquire().await?;
    let season = crate::seasons::current(&mut conn, s.config.season_days).await?;
    drop(conn);
    let (online_connections, online_players) = {
        let online = s.online.lock().unwrap();
        let players: std::collections::HashSet<i64> =
            online.values().map(|o| o.account_id).collect();
        (online.len(), players.len())
    };
    Ok(Json(Overview {
        accounts,
        admins,
        banned,
        new_24h,
        new_7d,
        active_24h,
        active_7d,
        matches,
        matches_24h,
        matches_7d,
        online_connections,
        online_players,
        in_match: s.busy.lock().unwrap().len(),
        pvp_queue: s.lobby.lock().unwrap().queued_pvp(),
        coins_total,
        items_total,
        clans,
        season: season.name,
        season_ends_at: season.ends_at,
        backups: s.config.backup_dir.is_some(),
        settings: s.settings.read().unwrap().clone(),
    }))
}

// ------------------------------------------------------------------ utenti

#[derive(Deserialize)]
struct UserQuery {
    #[serde(default)]
    q: String,
    #[serde(default)]
    offset: i64,
    #[serde(default)]
    limit: Option<i64>,
    /// "all" (predefinito), "banned", "admins", "online".
    #[serde(default)]
    filter: String,
}

#[derive(Serialize)]
struct UserRow {
    id: i64,
    username: String,
    elo: i64,
    coins: i64,
    created_at: i64,
    last_seen: Option<i64>,
    is_admin: bool,
    banned_until: Option<i64>,
    ban_reason: Option<String>,
    matches: i64,
    roster: i64,
    clan: Option<String>,
    online: bool,
}

#[derive(Serialize)]
struct UserPage {
    total: i64,
    users: Vec<UserRow>,
}

type UserTuple = (
    i64,
    String,
    i64,
    i64,
    i64,
    Option<i64>,
    bool,
    Option<i64>,
    Option<String>,
    i64,
    i64,
    Option<String>,
);

const USER_SELECT: &str = "SELECT a.id, a.username, a.elo, a.coins, a.created_at, a.last_seen, a.is_admin,
        a.banned_until, a.ban_reason,
        (SELECT COUNT(*) FROM match_players p WHERE p.account_id = a.id),
        (SELECT COUNT(*) FROM roster r WHERE r.account_id = a.id),
        (SELECT c.tag FROM clan_members m JOIN clans c ON c.id = m.clan_id WHERE m.account_id = a.id)
     FROM accounts a";

fn user_row(s: &AppState, r: UserTuple) -> UserRow {
    let online = s
        .online
        .lock()
        .unwrap()
        .values()
        .any(|o| o.account_id == r.0);
    let t = now();
    UserRow {
        id: r.0,
        username: r.1,
        elo: r.2,
        coins: r.3,
        created_at: r.4,
        last_seen: r.5,
        is_admin: r.6,
        banned_until: r.7.filter(|&u| u > t),
        ban_reason: r.8,
        matches: r.9,
        roster: r.10,
        clan: r.11,
        online,
    }
}

async fn users(
    State(s): State<AppState>,
    _a: AdminUser,
    Query(q): Query<UserQuery>,
) -> ApiResult<Json<UserPage>> {
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let pattern = format!("%{}%", q.q.trim());
    let t = now();
    let online_ids: Vec<i64> = s
        .online
        .lock()
        .unwrap()
        .values()
        .map(|o| o.account_id)
        .collect();
    let cond = match q.filter.as_str() {
        "banned" => "a.banned_until IS NOT NULL AND a.banned_until > ?2",
        "admins" => "a.is_admin = 1 AND ?2 = ?2",
        "online" => "a.id IN (SELECT value FROM json_each(?3)) AND ?2 = ?2",
        _ => "?2 = ?2",
    };
    let ids_json = serde_json::to_string(&online_ids).unwrap_or_else(|_| "[]".into());
    let where_ = format!("WHERE a.username LIKE ?1 AND {cond} AND ?3 = ?3");
    let total: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT COUNT(*) FROM accounts a {where_}"
    )))
    .bind(&pattern)
    .bind(t)
    .bind(&ids_json)
    .fetch_one(&s.db)
    .await?;
    let rows: Vec<UserTuple> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{USER_SELECT} {where_} ORDER BY COALESCE(a.last_seen, a.created_at) DESC LIMIT ?4 OFFSET ?5"
    )))
    .bind(&pattern)
    .bind(t)
    .bind(&ids_json)
    .bind(limit)
    .bind(q.offset.max(0))
    .fetch_all(&s.db)
    .await?;
    Ok(Json(UserPage {
        total,
        users: rows.into_iter().map(|r| user_row(&s, r)).collect(),
    }))
}

#[derive(Serialize)]
struct UserDetail {
    user: UserRow,
    sessions: i64,
    roster: Vec<crate::db::RosterUnit>,
    recent: Vec<AdminMatch>,
    log: Vec<LogRow>,
}

async fn user_detail(
    State(s): State<AppState>,
    _a: AdminUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<UserDetail>> {
    let row: UserTuple =
        sqlx::query_as(sqlx::AssertSqlSafe(format!("{USER_SELECT} WHERE a.id = ?")))
            .bind(id)
            .fetch_optional(&s.db)
            .await?
            .ok_or_else(|| ApiError::not_found("utente inesistente"))?;
    let username = row.1.clone();
    let sessions: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE account_id = ? AND expires_at > ?")
            .bind(id)
            .bind(now())
            .fetch_one(&s.db)
            .await?;
    let recent = match_rows(&s.db, Some(id), "", 0, 15).await?;
    let log = log_rows(&s.db, Some(&username), 0, 20).await?;
    Ok(Json(UserDetail {
        user: user_row(&s, row),
        sessions,
        roster: crate::db::roster(&s.db, id).await?,
        recent,
        log,
    }))
}

#[derive(Deserialize)]
struct Ban {
    /// Giorni di sospensione; assente o 0 = a tempo indeterminato.
    #[serde(default)]
    days: Option<i64>,
    #[serde(default)]
    reason: String,
}

async fn ban(
    State(s): State<AppState>,
    AdminUser(a): AdminUser,
    Path(id): Path<i64>,
    Json(b): Json<Ban>,
) -> ApiResult<Json<serde_json::Value>> {
    if id == a.id {
        return Err(ApiError::bad_request("non puoi sospendere te stesso"));
    }
    let name = username_of(&s.db, id).await?;
    let until = match b.days.filter(|&d| d > 0) {
        Some(d) => now() + d.min(36_500) * 86_400,
        None => PERMANENT,
    };
    let reason: String = b.reason.trim().chars().take(200).collect();
    sqlx::query("UPDATE accounts SET banned_until = ?, ban_reason = ? WHERE id = ?")
        .bind(until)
        .bind(&reason)
        .bind(id)
        .execute(&s.db)
        .await?;
    auth::revoke_sessions(&s.db, id).await?;
    let kicked = kick_account(&s, id);
    let span = if until == PERMANENT {
        "indeterminata".to_string()
    } else {
        format!("fino a {}", date(until))
    };
    record(
        &s.db,
        &a,
        "sospensione",
        Some(&name),
        &format!("{span}; motivo: {reason}"),
    )
    .await?;
    Ok(Json(
        serde_json::json!({"banned_until": until, "kicked": kicked}),
    ))
}

async fn unban(
    State(s): State<AppState>,
    AdminUser(a): AdminUser,
    Path(id): Path<i64>,
) -> ApiResult<()> {
    let name = username_of(&s.db, id).await?;
    sqlx::query("UPDATE accounts SET banned_until = NULL, ban_reason = NULL WHERE id = ?")
        .bind(id)
        .execute(&s.db)
        .await?;
    record(&s.db, &a, "riattivazione", Some(&name), "").await?;
    Ok(())
}

#[derive(Deserialize)]
struct Coins {
    delta: i64,
    #[serde(default)]
    reason: String,
}

async fn coins(
    State(s): State<AppState>,
    AdminUser(a): AdminUser,
    Path(id): Path<i64>,
    Json(c): Json<Coins>,
) -> ApiResult<Json<serde_json::Value>> {
    let name = username_of(&s.db, id).await?;
    if c.delta == 0 || c.delta.abs() > 10_000_000 {
        return Err(ApiError::bad_request("variazione non valida"));
    }
    let coins: i64 = sqlx::query_scalar(
        "UPDATE accounts SET coins = MAX(0, coins + ?) WHERE id = ? RETURNING coins",
    )
    .bind(c.delta)
    .bind(id)
    .fetch_one(&s.db)
    .await?;
    record(
        &s.db,
        &a,
        "monete",
        Some(&name),
        &format!("{:+} (ora {coins}); {}", c.delta, c.reason.trim()),
    )
    .await?;
    Ok(Json(serde_json::json!({"coins": coins})))
}

#[derive(Deserialize)]
struct NewPassword {
    password: String,
}

async fn password(
    State(s): State<AppState>,
    AdminUser(a): AdminUser,
    Path(id): Path<i64>,
    Json(p): Json<NewPassword>,
) -> ApiResult<()> {
    let name = username_of(&s.db, id).await?;
    if p.password.chars().count() < 8 {
        return Err(ApiError::bad_request(
            "la password deve avere almeno 8 caratteri",
        ));
    }
    let hash = auth::hash_password(p.password).await?;
    sqlx::query("UPDATE accounts SET password_hash = ? WHERE id = ?")
        .bind(hash)
        .bind(id)
        .execute(&s.db)
        .await?;
    auth::revoke_sessions(&s.db, id).await?;
    kick_account(&s, id);
    record(&s.db, &a, "nuova password", Some(&name), "sessioni chiuse").await?;
    Ok(())
}

async fn logout(
    State(s): State<AppState>,
    AdminUser(a): AdminUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    let name = username_of(&s.db, id).await?;
    let sessions = auth::revoke_sessions(&s.db, id).await?;
    let kicked = kick_account(&s, id);
    record(
        &s.db,
        &a,
        "disconnessione",
        Some(&name),
        &format!("{sessions} sessioni, {kicked} connessioni"),
    )
    .await?;
    Ok(Json(
        serde_json::json!({"sessions": sessions, "kicked": kicked}),
    ))
}

#[derive(Deserialize)]
struct Role {
    admin: bool,
}

async fn role(
    State(s): State<AppState>,
    AdminUser(a): AdminUser,
    Path(id): Path<i64>,
    Json(r): Json<Role>,
) -> ApiResult<()> {
    if id == a.id && !r.admin {
        return Err(ApiError::bad_request(
            "non puoi togliere il ruolo a te stesso",
        ));
    }
    let name = username_of(&s.db, id).await?;
    sqlx::query("UPDATE accounts SET is_admin = ? WHERE id = ?")
        .bind(r.admin)
        .bind(id)
        .execute(&s.db)
        .await?;
    record(
        &s.db,
        &a,
        if r.admin {
            "ruolo admin dato"
        } else {
            "ruolo admin tolto"
        },
        Some(&name),
        "",
    )
    .await?;
    Ok(())
}

// ------------------------------------------------------------------ connessioni

#[derive(Serialize)]
struct OnlineRow {
    conn: u64,
    account_id: i64,
    username: String,
    since: i64,
    in_match: bool,
}

async fn online(State(s): State<AppState>, _a: AdminUser) -> ApiResult<Json<Vec<OnlineRow>>> {
    let busy = s.busy.lock().unwrap().clone();
    let mut rows: Vec<OnlineRow> = s
        .online
        .lock()
        .unwrap()
        .iter()
        .map(|(&conn, o)| OnlineRow {
            conn,
            account_id: o.account_id,
            username: o.username.clone(),
            since: o.since,
            in_match: busy.contains(&o.account_id),
        })
        .collect();
    rows.sort_by_key(|r| r.since);
    Ok(Json(rows))
}

async fn kick(
    State(s): State<AppState>,
    AdminUser(a): AdminUser,
    Path(conn): Path<u64>,
) -> ApiResult<()> {
    let name = {
        let online = s.online.lock().unwrap();
        let o = online
            .get(&conn)
            .ok_or_else(|| ApiError::not_found("connessione già chiusa"))?;
        let _ = o.tx.send(ToConn::Kick);
        o.username.clone()
    };
    record(
        &s.db,
        &a,
        "connessione chiusa",
        Some(&name),
        &format!("connessione {conn}"),
    )
    .await?;
    Ok(())
}

// ------------------------------------------------------------------ partite

#[derive(Deserialize)]
struct MatchQuery {
    #[serde(default)]
    mode: String,
    #[serde(default)]
    offset: i64,
    #[serde(default)]
    limit: Option<i64>,
}

#[derive(Serialize)]
pub struct AdminMatch {
    id: i64,
    mode: String,
    biome: String,
    turns: i64,
    winner_team: Option<i64>,
    started_at: i64,
    ended_at: i64,
    /// "nome (squadra, esito)" per ogni giocatore umano.
    players: Vec<String>,
}

async fn match_rows(
    db: &SqlitePool,
    account: Option<i64>,
    mode: &str,
    offset: i64,
    limit: i64,
) -> sqlx::Result<Vec<AdminMatch>> {
    let rows: Vec<(i64, String, String, i64, Option<i64>, i64, i64, Option<String>)> = sqlx::query_as(
        "SELECT m.id, m.mode, m.biome, m.turns, m.winner_team, m.started_at, m.ended_at,
                (SELECT group_concat(a.username || ' (' || p.team || ', ' || p.result || ')', '|')
                   FROM match_players p JOIN accounts a ON a.id = p.account_id WHERE p.match_id = m.id)
         FROM matches m
         WHERE (?1 = '' OR m.mode = ?1)
           AND (?2 IS NULL OR EXISTS (SELECT 1 FROM match_players p WHERE p.match_id = m.id AND p.account_id = ?2))
         ORDER BY m.id DESC LIMIT ?3 OFFSET ?4",
    )
    .bind(mode)
    .bind(account)
    .bind(limit)
    .bind(offset.max(0))
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| AdminMatch {
            id: r.0,
            mode: r.1,
            biome: r.2,
            turns: r.3,
            winner_team: r.4,
            started_at: r.5,
            ended_at: r.6,
            players: r
                .7
                .map(|p| p.split('|').map(str::to_string).collect())
                .unwrap_or_default(),
        })
        .collect())
}

async fn matches(
    State(s): State<AppState>,
    _a: AdminUser,
    Query(q): Query<MatchQuery>,
) -> ApiResult<Json<Vec<AdminMatch>>> {
    Ok(Json(
        match_rows(
            &s.db,
            None,
            &q.mode,
            q.offset,
            q.limit.unwrap_or(50).clamp(1, 200),
        )
        .await?,
    ))
}

// ------------------------------------------------------------------ impostazioni

async fn get_settings(State(s): State<AppState>, _a: AdminUser) -> ApiResult<Json<Settings>> {
    Ok(Json(s.settings.read().unwrap().clone()))
}

async fn put_settings(
    State(s): State<AppState>,
    AdminUser(a): AdminUser,
    Json(new): Json<Settings>,
) -> ApiResult<Json<Settings>> {
    let new = new.clamped();
    let old = s.settings.read().unwrap().clone();
    new.save(&s.db).await?;
    *s.settings.write().unwrap() = new.clone();
    let (o, n) = (
        serde_json::to_value(&old).unwrap(),
        serde_json::to_value(&new).unwrap(),
    );
    let changes: Vec<String> = n
        .as_object()
        .unwrap()
        .iter()
        .filter(|(k, v)| o.get(*k) != Some(v))
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    if !changes.is_empty() {
        record(&s.db, &a, "impostazioni", None, &changes.join(", ")).await?;
    }
    Ok(Json(new))
}

// ------------------------------------------------------------------ stagioni e backup

async fn close_season(
    State(s): State<AppState>,
    AdminUser(a): AdminUser,
) -> ApiResult<Json<serde_json::Value>> {
    let t = now();
    sqlx::query("UPDATE seasons SET ends_at = ? WHERE closed = 0")
        .bind(t)
        .execute(&s.db)
        .await?;
    let closed = crate::seasons::rollover_if_due(&s.db, s.config.season_days, t).await?;
    let name = closed.as_ref().map(|c| c.name.clone()).unwrap_or_default();
    record(&s.db, &a, "stagione chiusa in anticipo", Some(&name), "").await?;
    Ok(Json(serde_json::json!({"closed": name})))
}

async fn backup(
    State(s): State<AppState>,
    AdminUser(a): AdminUser,
) -> ApiResult<Json<serde_json::Value>> {
    let Some(dir) = s.config.backup_dir.clone() else {
        return Err(ApiError::bad_request(
            "backup non configurati (TACTICA_BACKUP_DIR)",
        ));
    };
    let path = crate::db::backup(&s.db, &dir, s.config.backup_keep)
        .await
        .map_err(ApiError::internal)?;
    record(&s.db, &a, "backup", None, &path).await?;
    Ok(Json(serde_json::json!({"path": path})))
}

// ------------------------------------------------------------------ registro

#[derive(Serialize)]
pub struct LogRow {
    at: i64,
    admin: String,
    action: String,
    target: Option<String>,
    detail: Option<String>,
}

async fn log_rows(
    db: &SqlitePool,
    target: Option<&str>,
    offset: i64,
    limit: i64,
) -> sqlx::Result<Vec<LogRow>> {
    let rows: Vec<(i64, String, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT at, admin, action, target, detail FROM admin_log
         WHERE (?1 IS NULL OR target = ?1) ORDER BY id DESC LIMIT ?2 OFFSET ?3",
    )
    .bind(target)
    .bind(limit)
    .bind(offset.max(0))
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(at, admin, action, target, detail)| LogRow {
            at,
            admin,
            action,
            target,
            detail,
        })
        .collect())
}

#[derive(Deserialize)]
struct Page {
    #[serde(default)]
    offset: i64,
}

async fn log_list(
    State(s): State<AppState>,
    _a: AdminUser,
    Query(p): Query<Page>,
) -> ApiResult<Json<Vec<LogRow>>> {
    Ok(Json(log_rows(&s.db, None, p.offset, 100).await?))
}

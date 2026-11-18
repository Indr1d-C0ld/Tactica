//! Clan: creazione, adesione (aperta o su invito), ruoli, espulsioni.
//! I membri guadagnano punti stagionali per il clan e un bonus del 10% sulle monete.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};

use crate::AppState;
use crate::auth::AuthUser;
use crate::db::now;
use crate::error::{ApiError, ApiResult};
use crate::seasons;

pub const CREATE_COST: i64 = 200;
pub const MAX_MEMBERS: i64 = 20;
pub const COIN_BONUS_PCT: i64 = 10;

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct Membership {
    pub clan_id: i64,
    pub name: String,
    pub tag: String,
    pub role: String,
}

pub async fn membership(
    conn: &mut SqliteConnection,
    account: i64,
) -> sqlx::Result<Option<Membership>> {
    sqlx::query_as(
        "SELECT c.id AS clan_id, c.name, c.tag, m.role FROM clan_members m JOIN clans c ON c.id = m.clan_id
         WHERE m.account_id = ?",
    )
    .bind(account)
    .fetch_optional(conn)
    .await
}

async fn my_clan(s: &AppState, user: &AuthUser) -> ApiResult<Membership> {
    let mut conn = s.db.acquire().await?;
    membership(&mut conn, user.id)
        .await?
        .ok_or_else(|| ApiError::conflict("non fai parte di un clan"))
}

fn forbidden(msg: &str) -> ApiError {
    ApiError::new(StatusCode::FORBIDDEN, msg)
}

async fn clan_id_by_tag(db: &SqlitePool, tag: &str) -> ApiResult<(i64, bool)> {
    sqlx::query_as::<_, (i64, bool)>("SELECT id, open FROM clans WHERE tag = ?")
        .bind(tag)
        .fetch_optional(db)
        .await?
        .ok_or_else(|| ApiError::not_found("clan non trovato"))
}

async fn account_by_name(db: &SqlitePool, name: &str) -> ApiResult<i64> {
    sqlx::query_scalar("SELECT id FROM accounts WHERE username = ?")
        .bind(name)
        .fetch_optional(db)
        .await?
        .ok_or_else(|| ApiError::not_found("giocatore non trovato"))
}

// ---------------------------------------------------------------- consultazione

#[derive(Serialize)]
pub struct ClanSummary {
    tag: String,
    name: String,
    motto: String,
    open: bool,
    members: i64,
    season_points: i64,
    rating: i64,
    war_wins: i64,
    war_losses: i64,
}

pub async fn list(State(s): State<AppState>) -> ApiResult<Json<Vec<ClanSummary>>> {
    let mut conn = s.db.acquire().await?;
    let season = seasons::current(&mut conn, s.config.season_days).await?;
    let rows = sqlx::query_as::<_, (String, String, String, bool, i64, i64, i64, i64, i64)>(
        "SELECT c.tag, c.name, c.motto, c.open,
                (SELECT COUNT(*) FROM clan_members m WHERE m.clan_id = c.id),
                COALESCE((SELECT points FROM clan_season_points p WHERE p.clan_id = c.id AND p.season_id = ?), 0) AS pts,
                c.rating, c.war_wins, c.war_losses
         FROM clans c ORDER BY pts DESC, c.rating DESC, c.name LIMIT 100",
    )
    .bind(season.id)
    .fetch_all(&mut *conn)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(
                |(tag, name, motto, open, members, season_points, rating, war_wins, war_losses)| {
                    ClanSummary {
                        tag,
                        name,
                        motto,
                        open,
                        members,
                        season_points,
                        rating,
                        war_wins,
                        war_losses,
                    }
                },
            )
            .collect(),
    ))
}

#[derive(Serialize)]
pub struct Member {
    username: String,
    role: String,
    elo: i64,
    joined_at: i64,
}

#[derive(Serialize)]
pub struct PastSeason {
    number: i64,
    points: i64,
    rank: Option<i64>,
}

#[derive(Serialize)]
pub struct ClanDetail {
    tag: String,
    name: String,
    motto: String,
    open: bool,
    created_at: i64,
    members: Vec<Member>,
    season_points: i64,
    season_wins: i64,
    history: Vec<PastSeason>,
    rating: i64,
    war_wins: i64,
    war_losses: i64,
    war_draws: i64,
}

pub async fn detail(
    State(s): State<AppState>,
    Path(tag): Path<String>,
) -> ApiResult<Json<ClanDetail>> {
    let row = sqlx::query_as::<_, (i64, String, String, String, bool, i64, i64, i64, i64, i64)>(
        "SELECT id, tag, name, motto, open, created_at, rating, war_wins, war_losses, war_draws
         FROM clans WHERE tag = ?",
    )
    .bind(&tag)
    .fetch_optional(&s.db)
    .await?
    .ok_or_else(|| ApiError::not_found("clan non trovato"))?;
    let members = sqlx::query_as::<_, (String, String, i64, i64)>(
        "SELECT a.username, m.role, a.elo, m.joined_at FROM clan_members m JOIN accounts a ON a.id = m.account_id
         WHERE m.clan_id = ?
         ORDER BY CASE m.role WHEN 'leader' THEN 0 WHEN 'officer' THEN 1 ELSE 2 END, m.joined_at",
    )
    .bind(row.0)
    .fetch_all(&s.db)
    .await?
    .into_iter()
    .map(|(username, role, elo, joined_at)| Member { username, role, elo, joined_at })
    .collect();
    let mut conn = s.db.acquire().await?;
    let season = seasons::current(&mut conn, s.config.season_days).await?;
    let (season_points, season_wins): (i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(points), 0), COALESCE(SUM(wins), 0) FROM clan_season_points
         WHERE clan_id = ? AND season_id = ?",
    )
    .bind(row.0)
    .bind(season.id)
    .fetch_one(&mut *conn)
    .await?;
    let history = sqlx::query_as::<_, (i64, i64, Option<i64>)>(
        "SELECT s.number, p.points, p.final_rank FROM clan_season_points p JOIN seasons s ON s.id = p.season_id
         WHERE p.clan_id = ? AND s.closed = 1 ORDER BY s.number DESC",
    )
    .bind(row.0)
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .map(|(number, points, rank)| PastSeason { number, points, rank })
    .collect();
    Ok(Json(ClanDetail {
        tag: row.1,
        name: row.2,
        motto: row.3,
        open: row.4,
        created_at: row.5,
        members,
        season_points,
        season_wins,
        history,
        rating: row.6,
        war_wins: row.7,
        war_losses: row.8,
        war_draws: row.9,
    }))
}

// ---------------------------------------------------------------- azioni

#[derive(Deserialize)]
pub struct Create {
    name: String,
    tag: String,
    #[serde(default)]
    motto: String,
    #[serde(default = "yes")]
    open: bool,
}

fn yes() -> bool {
    true
}

fn valid_tag(t: &str) -> bool {
    (2..=5).contains(&t.len()) && t.chars().all(|c| c.is_ascii_alphanumeric())
}

pub async fn create(
    State(s): State<AppState>,
    user: AuthUser,
    Json(c): Json<Create>,
) -> ApiResult<Json<ClanDetail>> {
    let name = c.name.trim().to_string();
    if !(3..=24).contains(&name.chars().count()) || name.chars().any(char::is_control) {
        return Err(ApiError::bad_request("nome del clan: da 3 a 24 caratteri"));
    }
    if !valid_tag(&c.tag) {
        return Err(ApiError::bad_request("sigla: da 2 a 5 lettere o cifre"));
    }
    let motto: String = c.motto.trim().chars().take(80).collect();
    let mut tx = s.db.begin().await?;
    if membership(&mut tx, user.id).await?.is_some() {
        return Err(ApiError::conflict("fai già parte di un clan"));
    }
    let paid = sqlx::query("UPDATE accounts SET coins = coins - ? WHERE id = ? AND coins >= ?")
        .bind(CREATE_COST)
        .bind(user.id)
        .bind(CREATE_COST)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if paid == 0 {
        return Err(ApiError::conflict(format!(
            "servono {CREATE_COST} monete per fondare un clan"
        )));
    }
    let res = sqlx::query(
        "INSERT INTO clans (name, tag, motto, open, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&name)
    .bind(c.tag.to_uppercase())
    .bind(&motto)
    .bind(c.open)
    .bind(now())
    .execute(&mut *tx)
    .await;
    let clan = match res {
        Ok(r) => r.last_insert_rowid(),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            return Err(ApiError::conflict("nome o sigla già in uso"));
        }
        Err(e) => return Err(e.into()),
    };
    sqlx::query("INSERT INTO clan_members (clan_id, account_id, role, joined_at) VALUES (?, ?, 'leader', ?)")
        .bind(clan)
        .bind(user.id)
        .bind(now())
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    crate::achievements::record(&s.db, user.id, &[("clan_founded", 1)]).await;
    tracing::info!(clan = %name, by = %user.username, "clan fondato");
    detail(State(s), Path(c.tag.to_uppercase())).await
}

pub async fn join(
    State(s): State<AppState>,
    user: AuthUser,
    Path(tag): Path<String>,
) -> ApiResult<Json<ClanDetail>> {
    let (clan, open) = clan_id_by_tag(&s.db, &tag).await?;
    let mut tx = s.db.begin().await?;
    if membership(&mut tx, user.id).await?.is_some() {
        return Err(ApiError::conflict("fai già parte di un clan"));
    }
    let invited: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM clan_invites WHERE clan_id = ? AND account_id = ?)",
    )
    .bind(clan)
    .bind(user.id)
    .fetch_one(&mut *tx)
    .await?;
    if !open && !invited {
        return Err(forbidden("il clan è su invito"));
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM clan_members WHERE clan_id = ?")
        .bind(clan)
        .fetch_one(&mut *tx)
        .await?;
    if count >= MAX_MEMBERS {
        return Err(ApiError::conflict(format!(
            "il clan è al completo ({MAX_MEMBERS} membri)"
        )));
    }
    sqlx::query("INSERT INTO clan_members (clan_id, account_id, role, joined_at) VALUES (?, ?, 'member', ?)")
        .bind(clan)
        .bind(user.id)
        .bind(now())
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM clan_invites WHERE account_id = ?")
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    crate::achievements::record(&s.db, user.id, &[]).await;
    detail(State(s), Path(tag)).await
}

/// Esce dal clan; se esce il capo, il comando passa al membro più anziano
/// (preferendo gli ufficiali); se non resta nessuno il clan si scioglie.
pub async fn leave(State(s): State<AppState>, user: AuthUser) -> ApiResult<()> {
    let m = my_clan(&s, &user).await?;
    let mut tx = s.db.begin().await?;
    sqlx::query("DELETE FROM clan_members WHERE account_id = ?")
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    if m.role == "leader" {
        let heir: Option<i64> = sqlx::query_scalar(
            "SELECT account_id FROM clan_members WHERE clan_id = ?
             ORDER BY CASE role WHEN 'officer' THEN 0 ELSE 1 END, joined_at LIMIT 1",
        )
        .bind(m.clan_id)
        .fetch_optional(&mut *tx)
        .await?;
        match heir {
            Some(h) => {
                sqlx::query("UPDATE clan_members SET role = 'leader' WHERE account_id = ?")
                    .bind(h)
                    .execute(&mut *tx)
                    .await?;
            }
            None => {
                sqlx::query("DELETE FROM clans WHERE id = ?")
                    .bind(m.clan_id)
                    .execute(&mut *tx)
                    .await?;
            }
        }
    }
    tx.commit().await?;
    Ok(())
}

#[derive(Deserialize)]
pub struct Target {
    username: String,
    #[serde(default)]
    role: Option<String>,
}

fn ensure_same_clan(m: &Membership, tag: &str) -> ApiResult<()> {
    if !m.tag.eq_ignore_ascii_case(tag) {
        return Err(forbidden("non fai parte di questo clan"));
    }
    Ok(())
}

pub async fn invite(
    State(s): State<AppState>,
    user: AuthUser,
    Path(tag): Path<String>,
    Json(t): Json<Target>,
) -> ApiResult<()> {
    let m = my_clan(&s, &user).await?;
    ensure_same_clan(&m, &tag)?;
    if m.role == "member" {
        return Err(forbidden("solo capo e ufficiali possono invitare"));
    }
    let target = account_by_name(&s.db, &t.username).await?;
    sqlx::query(
        "INSERT OR IGNORE INTO clan_invites (clan_id, account_id, created_at) VALUES (?, ?, ?)",
    )
    .bind(m.clan_id)
    .bind(target)
    .bind(now())
    .execute(&s.db)
    .await?;
    Ok(())
}

pub async fn kick(
    State(s): State<AppState>,
    user: AuthUser,
    Path(tag): Path<String>,
    Json(t): Json<Target>,
) -> ApiResult<()> {
    let m = my_clan(&s, &user).await?;
    ensure_same_clan(&m, &tag)?;
    let target = account_by_name(&s.db, &t.username).await?;
    let mut conn = s.db.acquire().await?;
    let tm = membership(&mut conn, target)
        .await?
        .filter(|tm| tm.clan_id == m.clan_id)
        .ok_or_else(|| ApiError::not_found("non è un membro del clan"))?;
    let allowed = match (m.role.as_str(), tm.role.as_str()) {
        ("leader", r) => r != "leader",
        ("officer", "member") => true,
        _ => false,
    };
    if !allowed || target == user.id {
        return Err(forbidden("non puoi espellere questo membro"));
    }
    sqlx::query("DELETE FROM clan_members WHERE account_id = ?")
        .bind(target)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// Il capo nomina ufficiali o membri; assegnando "leader" cede il comando.
pub async fn set_role(
    State(s): State<AppState>,
    user: AuthUser,
    Path(tag): Path<String>,
    Json(t): Json<Target>,
) -> ApiResult<()> {
    let m = my_clan(&s, &user).await?;
    ensure_same_clan(&m, &tag)?;
    if m.role != "leader" {
        return Err(forbidden("solo il capo può cambiare i ruoli"));
    }
    let role = t.role.unwrap_or_default();
    if !["leader", "officer", "member"].contains(&role.as_str()) {
        return Err(ApiError::bad_request("ruolo: leader, officer o member"));
    }
    let target = account_by_name(&s.db, &t.username).await?;
    let mut tx = s.db.begin().await?;
    let n = sqlx::query("UPDATE clan_members SET role = ? WHERE account_id = ? AND clan_id = ?")
        .bind(&role)
        .bind(target)
        .bind(m.clan_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(ApiError::not_found("non è un membro del clan"));
    }
    if role == "leader" && target != user.id {
        sqlx::query("UPDATE clan_members SET role = 'officer' WHERE account_id = ?")
            .bind(user.id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

#[derive(Deserialize)]
pub struct Settings {
    motto: Option<String>,
    open: Option<bool>,
}

pub async fn update(
    State(s): State<AppState>,
    user: AuthUser,
    Path(tag): Path<String>,
    Json(c): Json<Settings>,
) -> ApiResult<Json<ClanDetail>> {
    let m = my_clan(&s, &user).await?;
    ensure_same_clan(&m, &tag)?;
    if m.role != "leader" {
        return Err(forbidden("solo il capo può modificare il clan"));
    }
    if let Some(motto) = c.motto {
        let motto: String = motto.trim().chars().take(80).collect();
        sqlx::query("UPDATE clans SET motto = ? WHERE id = ?")
            .bind(motto)
            .bind(m.clan_id)
            .execute(&s.db)
            .await?;
    }
    if let Some(open) = c.open {
        sqlx::query("UPDATE clans SET open = ? WHERE id = ?")
            .bind(open)
            .bind(m.clan_id)
            .execute(&s.db)
            .await?;
    }
    detail(State(s), Path(m.tag)).await
}

/// Inviti ricevuti dall'utente (sigle dei clan).
pub async fn invites(db: &SqlitePool, account: i64) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar(
        "SELECT c.tag FROM clan_invites i JOIN clans c ON c.id = i.clan_id WHERE i.account_id = ? ORDER BY i.created_at",
    )
    .bind(account)
    .fetch_all(db)
    .await
}

use argon2::Argon2;
use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use axum::Json;
use axum::extract::{FromRequestParts, State};
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

use crate::AppState;
use crate::ratelimit::{ClientIp, FAILURE_WINDOW, LOGIN_FAILURES, too_many};

const MINUTE: std::time::Duration = std::time::Duration::from_secs(60);
const HOUR: std::time::Duration = std::time::Duration::from_secs(3600);
use crate::db::now;
use crate::error::{ApiError, ApiResult};

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub id: i64,
    pub username: String,
    pub is_admin: bool,
}

#[derive(Deserialize)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

#[derive(Serialize, Deserialize)]
pub struct Session {
    pub token: String,
    pub username: String,
}

fn token_hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

fn new_token() -> String {
    let mut buf = [0u8; 32];
    getrandom::fill(&mut buf).expect("sorgente casuale del sistema");
    hex::encode(buf)
}

fn valid_username(u: &str) -> bool {
    (3..=20).contains(&u.len()) && u.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub async fn user_for_token(db: &SqlitePool, token: &str) -> sqlx::Result<Option<AuthUser>> {
    let t = now();
    let row: Option<(i64, String, bool)> = sqlx::query_as(
        "SELECT a.id, a.username, a.is_admin FROM sessions s JOIN accounts a ON a.id = s.account_id
         WHERE s.token_hash = ? AND s.expires_at > ?
           AND (a.banned_until IS NULL OR a.banned_until <= ?)",
    )
    .bind(token_hash(token))
    .bind(t)
    .bind(t)
    .fetch_optional(db)
    .await?;
    if let Some((id, ..)) = &row {
        // Ultimo accesso, aggiornato al più una volta al minuto.
        sqlx::query(
            "UPDATE accounts SET last_seen = ? WHERE id = ? AND (last_seen IS NULL OR last_seen < ?)",
        )
        .bind(t)
        .bind(id)
        .bind(t - 60)
        .execute(db)
        .await?;
    }
    Ok(row.map(|(id, username, is_admin)| AuthUser {
        id,
        username,
        is_admin,
    }))
}

/// Hash Argon2 di una password (in un thread a parte: è lento di proposito).
pub async fn hash_password(password: String) -> ApiResult<String> {
    tokio::task::spawn_blocking(move || {
        Argon2::default()
            .hash_password(password.as_bytes())
            .map(|h| h.to_string())
    })
    .await
    .map_err(ApiError::internal)?
    .map_err(ApiError::internal)
}

/// Le sessioni di un account (tutte, o tutte tranne quella col token dato).
pub async fn revoke_sessions(db: &SqlitePool, account: i64) -> sqlx::Result<u64> {
    Ok(sqlx::query("DELETE FROM sessions WHERE account_id = ?")
        .bind(account)
        .execute(db)
        .await?
        .rows_affected())
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(ApiError::unauthorized)?;
        user_for_token(&state.db, token)
            .await?
            .ok_or_else(ApiError::unauthorized)
    }
}

async fn issue_session(
    state: &AppState,
    account: i64,
    username: String,
) -> ApiResult<Json<Session>> {
    let token = new_token();
    let expires = now() + state.config.session_days * 86_400;
    sqlx::query("DELETE FROM sessions WHERE expires_at <= ?")
        .bind(now())
        .execute(&state.db)
        .await?;
    sqlx::query("INSERT INTO sessions (token_hash, account_id, expires_at) VALUES (?, ?, ?)")
        .bind(token_hash(&token))
        .bind(account)
        .bind(expires)
        .execute(&state.db)
        .await?;
    Ok(Json(Session { token, username }))
}

pub async fn register(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Json(c): Json<Credentials>,
) -> ApiResult<Json<Session>> {
    let lim = &state.limiter;
    if !lim.hit(&format!("auth:{ip}"), state.config.auth_per_minute, MINUTE) {
        return Err(too_many());
    }
    let (open, start_coins) = {
        let st = state.settings.read().unwrap();
        (st.registrations_open, st.start_coins)
    };
    if !open {
        return Err(ApiError::new(
            axum::http::StatusCode::FORBIDDEN,
            "le registrazioni sono chiuse per ora",
        ));
    }
    if !valid_username(&c.username) {
        return Err(ApiError::bad_request(
            "nome utente: 3-20 caratteri, lettere, cifre o _",
        ));
    }
    if c.password.chars().count() < 8 {
        return Err(ApiError::bad_request(
            "la password deve avere almeno 8 caratteri",
        ));
    }
    // Un nome o una password non validi non consumano il limite orario.
    if !lim.hit(&format!("reg:{ip}"), state.config.register_per_hour, HOUR) {
        return Err(too_many());
    }
    let hash = hash_password(c.password.clone()).await?;
    let is_admin = state
        .config
        .admins
        .iter()
        .any(|a| a.eq_ignore_ascii_case(&c.username));
    let res = sqlx::query(
        "INSERT INTO accounts (username, password_hash, created_at, coins, is_admin) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&c.username)
    .bind(hash)
    .bind(now())
    .bind(start_coins)
    .bind(is_admin)
    .execute(&state.db)
    .await;
    let id = match res {
        Ok(r) => r.last_insert_rowid(),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            return Err(ApiError::conflict("nome utente già in uso"));
        }
        Err(e) => return Err(e.into()),
    };
    tracing::info!(user = %c.username, "nuovo account");
    issue_session(&state, id, c.username).await
}

#[allow(clippy::type_complexity)]
pub async fn login(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Json(c): Json<Credentials>,
) -> ApiResult<Json<Session>> {
    let lim = state.limiter.clone();
    if !lim.hit(&format!("auth:{ip}"), state.config.auth_per_minute, MINUTE) {
        return Err(too_many());
    }
    let fail_key = format!("fail:{}", c.username.to_lowercase());
    if lim.exceeded(&fail_key, LOGIN_FAILURES, FAILURE_WINDOW) {
        return Err(too_many());
    }
    let row: Option<(i64, String, String, Option<i64>, Option<String>)> = sqlx::query_as(
        "SELECT id, username, password_hash, banned_until, ban_reason FROM accounts WHERE username = ?",
    )
    .bind(&c.username)
    .fetch_optional(&state.db)
    .await?;
    let wrong = || {
        ApiError::new(
            axum::http::StatusCode::UNAUTHORIZED,
            "credenziali non valide",
        )
    };
    let Some((id, username, hash, banned_until, ban_reason)) = row else {
        lim.hit(&fail_key, LOGIN_FAILURES, FAILURE_WINDOW);
        return Err(wrong());
    };
    let ok = tokio::task::spawn_blocking(move || {
        PasswordHash::new(&hash)
            .map(|h| {
                Argon2::default()
                    .verify_password(c.password.as_bytes(), &h)
                    .is_ok()
            })
            .unwrap_or(false)
    })
    .await
    .map_err(ApiError::internal)?;
    if !ok {
        if !lim.hit(&fail_key, LOGIN_FAILURES, FAILURE_WINDOW) {
            tracing::warn!(user = %username, %ip, "accesso bloccato: troppe password sbagliate");
        }
        return Err(wrong());
    }
    lim.reset(&fail_key);
    if let Some(until) = banned_until.filter(|&u| u > now()) {
        let quando = if until >= crate::admin::PERMANENT {
            "a tempo indeterminato".to_string()
        } else {
            format!("fino a {}", crate::admin::date(until))
        };
        let motivo = ban_reason
            .filter(|r| !r.is_empty())
            .map(|r| format!(": {r}"))
            .unwrap_or_default();
        return Err(ApiError::new(
            axum::http::StatusCode::FORBIDDEN,
            format!("account sospeso {quando}{motivo}"),
        ));
    }
    issue_session(&state, id, username).await
}

pub async fn logout(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> ApiResult<()> {
    if let Some(token) = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
            .bind(token_hash(token))
            .execute(&state.db)
            .await?;
    }
    Ok(())
}

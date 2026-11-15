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
use crate::db::now;
use crate::error::{ApiError, ApiResult};

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub id: i64,
    pub username: String,
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
    let row: Option<(i64, String)> = sqlx::query_as(
        "SELECT a.id, a.username FROM sessions s JOIN accounts a ON a.id = s.account_id
         WHERE s.token_hash = ? AND s.expires_at > ?",
    )
    .bind(token_hash(token))
    .bind(now())
    .fetch_optional(db)
    .await?;
    Ok(row.map(|(id, username)| AuthUser { id, username }))
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
    Json(c): Json<Credentials>,
) -> ApiResult<Json<Session>> {
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
    let password = c.password.clone();
    let hash = tokio::task::spawn_blocking(move || {
        Argon2::default()
            .hash_password(password.as_bytes())
            .map(|h| h.to_string())
    })
    .await
    .map_err(ApiError::internal)?
    .map_err(ApiError::internal)?;

    let res =
        sqlx::query("INSERT INTO accounts (username, password_hash, created_at) VALUES (?, ?, ?)")
            .bind(&c.username)
            .bind(hash)
            .bind(now())
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

pub async fn login(
    State(state): State<AppState>,
    Json(c): Json<Credentials>,
) -> ApiResult<Json<Session>> {
    let row: Option<(i64, String, String)> =
        sqlx::query_as("SELECT id, username, password_hash FROM accounts WHERE username = ?")
            .bind(&c.username)
            .fetch_optional(&state.db)
            .await?;
    let wrong = || {
        ApiError::new(
            axum::http::StatusCode::UNAUTHORIZED,
            "credenziali non valide",
        )
    };
    let Some((id, username, hash)) = row else {
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
        return Err(wrong());
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

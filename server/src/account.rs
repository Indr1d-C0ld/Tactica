//! Gestione del proprio account: cambio della password e cancellazione
//! (con tutti i dati: roster, inventario, statistiche personali).

use argon2::Argon2;
use argon2::password_hash::PasswordVerifier;
use argon2::password_hash::phc::PasswordHash;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{delete, put};
use axum::{Json, Router};
use serde::Deserialize;

use crate::AppState;
use crate::auth::{self, AuthUser, Session};
use crate::error::{ApiError, ApiResult};
use crate::lobby::ToConn;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/me/password", put(change_password))
        .route("/api/me", delete(delete_account))
}

/// Verifica la password attuale dell'account.
async fn verify(s: &AppState, id: i64, password: String) -> ApiResult<()> {
    let hash: String = sqlx::query_scalar("SELECT password_hash FROM accounts WHERE id = ?")
        .bind(id)
        .fetch_one(&s.db)
        .await?;
    let ok = tokio::task::spawn_blocking(move || {
        PasswordHash::new(&hash)
            .map(|h| {
                Argon2::default()
                    .verify_password(password.as_bytes(), &h)
                    .is_ok()
            })
            .unwrap_or(false)
    })
    .await
    .map_err(ApiError::internal)?;
    if ok {
        Ok(())
    } else {
        Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "password attuale errata",
        ))
    }
}

#[derive(Deserialize)]
struct ChangePassword {
    old: String,
    new: String,
}

/// Cambia la password: chiude tutte le sessioni e ne apre una nuova.
async fn change_password(
    State(s): State<AppState>,
    user: AuthUser,
    Json(c): Json<ChangePassword>,
) -> ApiResult<Json<Session>> {
    if c.new.chars().count() < 8 {
        return Err(ApiError::bad_request(
            "la nuova password deve avere almeno 8 caratteri",
        ));
    }
    verify(&s, user.id, c.old).await?;
    let hash = auth::hash_password(c.new).await?;
    sqlx::query("UPDATE accounts SET password_hash = ? WHERE id = ?")
        .bind(hash)
        .bind(user.id)
        .execute(&s.db)
        .await?;
    auth::revoke_sessions(&s.db, user.id).await?;
    tracing::info!(user = %user.username, "password cambiata");
    auth::new_session(&s, user.id, user.username).await
}

#[derive(Deserialize)]
struct DeleteAccount {
    password: String,
}

/// Cancella l'account. Dal clan si esce come con «Lascia» (al capo succede
/// un ufficiale o il membro più anziano; un clan vuoto si scioglie).
async fn delete_account(
    State(s): State<AppState>,
    user: AuthUser,
    Json(d): Json<DeleteAccount>,
) -> ApiResult<()> {
    verify(&s, user.id, d.password).await?;
    if s.busy.lock().unwrap().contains(&user.id) {
        return Err(ApiError::conflict(
            "finisci la partita in corso prima di cancellare l'account",
        ));
    }
    // Non essere in un clan non è un errore.
    let _ = crate::clans::leave(State(s.clone()), user.clone()).await;
    sqlx::query("DELETE FROM accounts WHERE id = ?")
        .bind(user.id)
        .execute(&s.db)
        .await?;
    for o in s.online.lock().unwrap().values() {
        if o.account_id == user.id {
            let _ = o.tx.send(ToConn::Kick);
        }
    }
    tracing::info!(user = %user.username, "account cancellato dal titolare");
    Ok(())
}

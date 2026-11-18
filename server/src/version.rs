//! Compatibilità fra client e server. Il client manda la sua versione
//! (`regole.protocollo`) nell'header `X-Tactica-Version` e, per il
//! WebSocket, nel parametro `v`. Se non coincide con quella del server la
//! richiesta viene rifiutata con 426 e un messaggio che invita ad aggiornare.
//! Senza versione (curl, strumenti, test) la richiesta passa.

use axum::Json;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use crate::error::ApiError;

pub const HEADER: &str = "x-tactica-version";

pub fn check(client: Option<&str>) -> Result<(), ApiError> {
    match client {
        Some(v) if v != tactica_core::compat_version() => Err(ApiError::new(
            StatusCode::UPGRADE_REQUIRED,
            format!(
                "versione del gioco non compatibile con il server ({v} invece di {}): scarica l'ultima versione",
                tactica_core::compat_version()
            ),
        )),
        _ => Ok(()),
    }
}

/// Middleware per tutte le rotte HTTP.
pub async fn layer(req: Request, next: Next) -> Response {
    let v = req
        .headers()
        .get(HEADER)
        .and_then(|h| h.to_str().ok())
        .map(str::to_string);
    if let Err(e) = check(v.as_deref()) {
        return e.into_response();
    }
    next.run(req).await
}

#[derive(Serialize)]
pub struct Info {
    pub version: String,
    pub rules: u32,
    pub protocol: u32,
}

/// `GET /api/version`: versione richiesta dal server.
pub async fn info() -> Json<Info> {
    Json(Info {
        version: tactica_core::compat_version(),
        rules: tactica_core::RULES_VERSION,
        protocol: tactica_core::PROTOCOL_VERSION,
    })
}

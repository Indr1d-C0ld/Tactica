//! Limiti ai tentativi (in memoria): proteggono accesso e registrazione da
//! tentativi a raffica e da attacchi a forza bruta sulle password.
//!
//! - per indirizzo IP: richieste di accesso o registrazione al minuto
//!   (`TACTICA_AUTH_PER_MINUTE`, predefinito 30);
//! - per indirizzo IP: registrazioni all'ora (`TACTICA_REGISTER_PER_HOUR`, predefinito 5);
//! - per nome utente: dopo 5 password sbagliate in 5 minuti l'accesso è bloccato
//!   fino allo scadere della finestra (anche con la password giusta).
//!
//! L'IP è quello della connessione; se la connessione arriva da localhost
//! (il reverse proxy Caddy) si usa il primo indirizzo di `X-Forwarded-For`.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::StatusCode;
use axum::http::request::Parts;

use crate::error::ApiError;

pub const LOGIN_FAILURES: u32 = 5;
pub const FAILURE_WINDOW: Duration = Duration::from_secs(300);

#[derive(Default)]
pub struct Limiter {
    buckets: Mutex<HashMap<String, (u32, Instant)>>,
}

impl Limiter {
    /// Conta un tentativo; `false` se il limite della finestra è superato.
    pub fn hit(&self, key: &str, max: u32, window: Duration) -> bool {
        let now = Instant::now();
        let mut b = self.buckets.lock().unwrap();
        if b.len() > 50_000 {
            b.retain(|_, (_, start)| now.duration_since(*start) < Duration::from_secs(3600));
        }
        let e = b.entry(key.to_string()).or_insert((0, now));
        if now.duration_since(e.1) >= window {
            *e = (0, now);
        }
        e.0 += 1;
        e.0 <= max
    }

    /// Il limite è già superato? (senza contare un nuovo tentativo)
    pub fn exceeded(&self, key: &str, max: u32, window: Duration) -> bool {
        let b = self.buckets.lock().unwrap();
        b.get(key)
            .is_some_and(|(n, start)| *n >= max && Instant::now().duration_since(*start) < window)
    }

    pub fn reset(&self, key: &str) {
        self.buckets.lock().unwrap().remove(key);
    }
}

pub fn too_many() -> ApiError {
    ApiError::new(
        StatusCode::TOO_MANY_REQUESTS,
        "troppi tentativi: riprova tra qualche minuto",
    )
}

/// Indirizzo del client, con fiducia in `X-Forwarded-For` solo da localhost.
pub struct ClientIp(pub String);

impl<S: Send + Sync> FromRequestParts<S> for ClientIp {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let peer = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|c| c.0.ip());
        let forwarded = parts
            .headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());
        let ip = match (peer, forwarded) {
            (Some(p), Some(f)) if p.is_loopback() => f,
            (Some(p), _) => p.to_string(),
            (None, _) => "sconosciuto".to_string(),
        };
        Ok(ClientIp(ip))
    }
}

//! Impostazioni di gioco modificabili a caldo dal pannello admin (tabella
//! `settings`, una riga per chiave). Restano in memoria in `AppState`.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// Nuove registrazioni consentite.
    pub registrations_open: bool,
    /// Manutenzione: solo gli admin possono avviare partite.
    pub maintenance: bool,
    /// Messaggio mostrato a tutti nel menu (vuoto = nessuno).
    pub motd: String,
    /// Monete dei nuovi account.
    pub start_coins: i64,
    /// Percentuale delle monete di fine partita (100 = normali).
    pub coins_pct: i64,
    /// Percentuale dell'esperienza di fine partita (100 = normale).
    pub xp_pct: i64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            registrations_open: true,
            maintenance: false,
            motd: String::new(),
            start_coins: 400,
            coins_pct: 100,
            xp_pct: 100,
        }
    }
}

impl Settings {
    /// Valori entro limiti sensati.
    pub fn clamped(mut self) -> Self {
        self.start_coins = self.start_coins.clamp(0, 1_000_000);
        self.coins_pct = self.coins_pct.clamp(0, 1000);
        self.xp_pct = self.xp_pct.clamp(0, 1000);
        self.motd = self.motd.chars().take(300).collect();
        self
    }

    pub async fn load(db: &SqlitePool) -> sqlx::Result<Self> {
        let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value FROM settings")
            .fetch_all(db)
            .await?;
        let mut map = serde_json::to_value(Settings::default()).expect("impostazioni");
        for (k, v) in rows {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&v) {
                map[k] = val;
            }
        }
        Ok(serde_json::from_value::<Settings>(map)
            .unwrap_or_default()
            .clamped())
    }

    pub async fn save(&self, db: &SqlitePool) -> sqlx::Result<()> {
        let map = serde_json::to_value(self).expect("impostazioni");
        let mut tx = db.begin().await?;
        for (k, v) in map.as_object().expect("oggetto") {
            sqlx::query(
                "INSERT INTO settings (key, value) VALUES (?, ?)
                         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            )
            .bind(k)
            .bind(v.to_string())
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await
    }
}

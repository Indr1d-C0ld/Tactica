use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    /// Indirizzo di ascolto, es. `127.0.0.1:7777` (dietro reverse proxy TLS).
    pub addr: String,
    /// URL SQLite, es. `sqlite:///var/lib/tactica/tactica.db` o `sqlite::memory:`.
    pub db_url: String,
    /// Secondi a disposizione per turno prima che l'IA giochi al posto del giocatore.
    pub turn_secs: u64,
    /// Pausa tra le azioni dell'IA, per renderle seguibili dal client.
    pub ai_delay_ms: u64,
    /// Durata delle sessioni di accesso.
    pub session_days: i64,
    /// Cartella dei backup periodici del database (`None` = disattivati).
    pub backup_dir: Option<String>,
    pub backup_every_hours: u64,
    pub backup_keep: usize,
    /// Durata di una stagione classificata.
    pub season_days: i64,
    /// Giocatori minimi per avviare una missione cooperativa.
    pub coop_min_players: usize,
    /// Giocatori minimi per lato in una guerra fra clan.
    pub war_min_players: usize,
    /// Richieste di accesso o registrazione al minuto per indirizzo IP.
    pub auth_per_minute: u32,
    /// Registrazioni all'ora per indirizzo IP.
    pub register_per_hour: u32,
    /// Ore a disposizione per giocare un incontro di torneo.
    pub tournament_match_hours: i64,
    /// Nomi utente che all'avvio diventano amministratori (`TACTICA_ADMINS`, separati da virgole).
    pub admins: Vec<String>,
}

impl Config {
    pub fn from_env() -> Self {
        let num = |k: &str, d: u64| env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
        Config {
            addr: env::var("TACTICA_ADDR").unwrap_or_else(|_| "127.0.0.1:7777".into()),
            db_url: env::var("TACTICA_DB").unwrap_or_else(|_| "sqlite://tactica.db".into()),
            turn_secs: num("TACTICA_TURN_SECS", 60),
            ai_delay_ms: num("TACTICA_AI_DELAY_MS", 400),
            session_days: num("TACTICA_SESSION_DAYS", 30) as i64,
            backup_dir: env::var("TACTICA_BACKUP_DIR")
                .ok()
                .filter(|s| !s.is_empty()),
            backup_every_hours: num("TACTICA_BACKUP_EVERY_HOURS", 24).max(1),
            backup_keep: num("TACTICA_BACKUP_KEEP", 14) as usize,
            season_days: num("TACTICA_SEASON_DAYS", 28).max(1) as i64,
            coop_min_players: num("TACTICA_COOP_MIN_PLAYERS", 2).clamp(1, 6) as usize,
            war_min_players: num("TACTICA_WAR_MIN_PLAYERS", 1).clamp(1, 6) as usize,
            auth_per_minute: num("TACTICA_AUTH_PER_MINUTE", 30).max(1) as u32,
            register_per_hour: num("TACTICA_REGISTER_PER_HOUR", 5).max(1) as u32,
            tournament_match_hours: num("TACTICA_TOURNAMENT_MATCH_HOURS", 72).clamp(1, 24 * 60)
                as i64,
            admins: env::var("TACTICA_ADMINS")
                .unwrap_or_default()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        }
    }
}

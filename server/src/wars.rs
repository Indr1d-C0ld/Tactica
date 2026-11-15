//! Guerre fra clan: dichiarazione, accettazione, schieramenti, esito.
//!
//! Flusso: un capo o ufficiale dichiara guerra (`pending`); il clan sfidato
//! accetta (`accepted`) o rifiuta; i membri si schierano via WebSocket e,
//! quando entrambi i comandanti sono pronti, la battaglia parte (`live`).
//! A fine partita (`done`) si aggiornano ELO di clan, record e punti stagionali.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};

use crate::AppState;
use crate::auth::AuthUser;
use crate::clans::{self, Membership};
use crate::db::now;
use crate::error::{ApiError, ApiResult};
use crate::seasons;

pub const CLAN_ELO_K: f64 = 32.0;
pub const CLAN_POINTS_WAR_WIN: i64 = 5;
pub const WAR_WIN_COINS: i64 = 200;
pub const WAR_LOSS_COINS: i64 = 60;
pub const WAR_DRAW_COINS: i64 = 100;

/// Dati della guerra trasmessi ai giocatori e usati nel salvataggio.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WarInfo {
    pub id: i64,
    /// Indice = squadra (0: sfidante, 1: sfidato).
    pub clan_ids: [i64; 2],
    pub tags: [String; 2],
    pub names: [String; 2],
    /// Nome del torneo, se la guerra è un incontro di torneo.
    #[serde(default)]
    pub tournament: Option<String>,
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct WarRow {
    pub id: i64,
    pub status: String,
    pub created_at: i64,
    pub challenger_tag: String,
    pub challenger_name: String,
    pub defender_tag: String,
    pub defender_name: String,
    pub winner_tag: Option<String>,
    pub challenger_before: Option<i64>,
    pub challenger_after: Option<i64>,
    pub defender_before: Option<i64>,
    pub defender_after: Option<i64>,
    pub match_id: Option<i64>,
}

const WAR_SELECT: &str = "SELECT w.id, w.status, w.created_at,
        a.tag AS challenger_tag, a.name AS challenger_name,
        b.tag AS defender_tag, b.name AS defender_name,
        c.tag AS winner_tag,
        w.challenger_before, w.challenger_after, w.defender_before, w.defender_after, w.match_id
     FROM clan_wars w JOIN clans a ON a.id = w.challenger_id JOIN clans b ON b.id = w.defender_id
     LEFT JOIN clans c ON c.id = w.winner_id";

fn forbidden(m: &str) -> ApiError {
    ApiError::new(StatusCode::FORBIDDEN, m)
}

async fn captain(s: &AppState, user: &AuthUser) -> ApiResult<Membership> {
    let mut conn = s.db.acquire().await?;
    let m = clans::membership(&mut conn, user.id)
        .await?
        .ok_or_else(|| ApiError::conflict("non fai parte di un clan"))?;
    if m.role == "member" {
        return Err(forbidden("solo capo e ufficiali decidono le guerre"));
    }
    Ok(m)
}

#[derive(Deserialize)]
pub struct Declare {
    tag: String,
}

pub async fn declare(
    State(s): State<AppState>,
    user: AuthUser,
    Json(d): Json<Declare>,
) -> ApiResult<Json<WarRow>> {
    let me = captain(&s, &user).await?;
    let target: (i64,) = sqlx::query_as("SELECT id FROM clans WHERE tag = ?")
        .bind(&d.tag)
        .fetch_optional(&s.db)
        .await?
        .ok_or_else(|| ApiError::not_found("clan non trovato"))?;
    if target.0 == me.clan_id {
        return Err(ApiError::bad_request(
            "non puoi dichiarare guerra al tuo clan",
        ));
    }
    let open: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM clan_wars WHERE status IN ('pending', 'accepted', 'live')
           AND ((challenger_id = ? AND defender_id = ?) OR (challenger_id = ? AND defender_id = ?)))",
    )
    .bind(me.clan_id)
    .bind(target.0)
    .bind(target.0)
    .bind(me.clan_id)
    .fetch_one(&s.db)
    .await?;
    if open {
        return Err(ApiError::conflict(
            "c'è già una guerra aperta fra questi clan",
        ));
    }
    let id = sqlx::query(
        "INSERT INTO clan_wars (challenger_id, defender_id, status, created_at, updated_at)
         VALUES (?, ?, 'pending', ?, ?)",
    )
    .bind(me.clan_id)
    .bind(target.0)
    .bind(now())
    .bind(now())
    .execute(&s.db)
    .await?
    .last_insert_rowid();
    tracing::info!(war = id, from = %me.tag, to = %d.tag, "guerra dichiarata");
    Ok(Json(row(&s.db, id).await?))
}

pub async fn row(db: &SqlitePool, id: i64) -> ApiResult<WarRow> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!("{WAR_SELECT} WHERE w.id = ?")))
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or_else(|| ApiError::not_found("guerra non trovata"))
}

/// Guerre del proprio clan: aperte e ultime concluse.
pub async fn list(State(s): State<AppState>, user: AuthUser) -> ApiResult<Json<Vec<WarRow>>> {
    let mut conn = s.db.acquire().await?;
    let Some(m) = clans::membership(&mut conn, user.id).await? else {
        return Ok(Json(Vec::new()));
    };
    // Testo SQL costante (nessun dato dell'utente nel formato).
    let rows = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{WAR_SELECT} WHERE w.challenger_id = ? OR w.defender_id = ?
         ORDER BY CASE w.status WHEN 'live' THEN 0 WHEN 'accepted' THEN 1 WHEN 'pending' THEN 2 ELSE 3 END,
                  w.updated_at DESC LIMIT 30"
    )))
    .bind(m.clan_id)
    .bind(m.clan_id)
    .fetch_all(&mut *conn)
    .await?;
    Ok(Json(rows))
}

async fn transition(
    s: &AppState,
    user: &AuthUser,
    id: i64,
    who: &str,
    from: &[&str],
    to: &str,
) -> ApiResult<Json<WarRow>> {
    let me = captain(s, user).await?;
    let (challenger, defender, status, tournament): (i64, i64, String, Option<i64>) =
        sqlx::query_as(
            "SELECT challenger_id, defender_id, status, tournament_id FROM clan_wars WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or_else(|| ApiError::not_found("guerra non trovata"))?;
    if tournament.is_some() {
        return Err(ApiError::conflict(
            "gli incontri di torneo non si annullano né si rifiutano",
        ));
    }
    let allowed = match who {
        "defender" => me.clan_id == defender,
        _ => me.clan_id == defender || me.clan_id == challenger,
    };
    if !allowed {
        return Err(forbidden("questa decisione non spetta al tuo clan"));
    }
    if !from.contains(&status.as_str()) {
        return Err(ApiError::conflict(format!(
            "la guerra è nello stato «{status}»"
        )));
    }
    sqlx::query("UPDATE clan_wars SET status = ?, updated_at = ? WHERE id = ?")
        .bind(to)
        .bind(now())
        .bind(id)
        .execute(&s.db)
        .await?;
    Ok(Json(row(&s.db, id).await?))
}

pub async fn accept(
    State(s): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<WarRow>> {
    transition(&s, &user, id, "defender", &["pending"], "accepted").await
}

pub async fn decline(
    State(s): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<WarRow>> {
    transition(&s, &user, id, "defender", &["pending"], "declined").await
}

pub async fn cancel(
    State(s): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<WarRow>> {
    let r = transition(
        &s,
        &user,
        id,
        "either",
        &["pending", "accepted"],
        "cancelled",
    )
    .await?;
    // Chiude anche l'eventuale schieramento in corso.
    if let Some(room) = s.lobby.lock().unwrap().close_war_room(id) {
        for m in room.sides.iter().flatten() {
            m.send(crate::protocol::ServerMsg::WarRoom { room: None });
        }
    }
    Ok(r)
}

/// Per lo schieramento: la guerra deve essere accettata e l'utente deve
/// appartenere a uno dei due clan. Restituisce informazioni e lato (squadra).
pub async fn for_join(
    conn: &mut SqliteConnection,
    war: i64,
    account: i64,
) -> Result<(WarInfo, usize, String), String> {
    #[allow(clippy::type_complexity)]
    let row: Option<(i64, i64, String, String, String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT w.challenger_id, w.defender_id, w.status, a.tag, a.name, b.tag, b.name, t.name
         FROM clan_wars w JOIN clans a ON a.id = w.challenger_id JOIN clans b ON b.id = w.defender_id
         LEFT JOIN tournaments t ON t.id = w.tournament_id
         WHERE w.id = ?",
    )
    .bind(war)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| e.to_string())?;
    let Some((ch, df, status, at, an, bt, bn, tournament)) = row else {
        return Err("guerra non trovata".into());
    };
    if status != "accepted" {
        return Err(format!("la guerra non è pronta (stato «{status}»)"));
    }
    let m = clans::membership(conn, account)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("non fai parte di un clan")?;
    let side = if m.clan_id == ch {
        0
    } else if m.clan_id == df {
        1
    } else {
        return Err("il tuo clan non partecipa a questa guerra".into());
    };
    let info = WarInfo {
        id: war,
        clan_ids: [ch, df],
        tags: [at, bt],
        names: [an, bn],
        tournament,
    };
    Ok((info, side, m.role))
}

pub async fn set_status(db: &SqlitePool, war: i64, status: &str) -> sqlx::Result<()> {
    sqlx::query("UPDATE clan_wars SET status = ?, updated_at = ? WHERE id = ?")
        .bind(status)
        .bind(now())
        .bind(war)
        .execute(db)
        .await?;
    Ok(())
}

/// Esito: ELO di clan (somma zero), record, punti stagionali al vincitore.
pub async fn record_result(
    conn: &mut SqliteConnection,
    war: &WarInfo,
    winner_team: Option<u8>,
    match_id: i64,
    season: &seasons::Season,
) -> sqlx::Result<()> {
    let mut rating = [0i64; 2];
    for (i, id) in war.clan_ids.iter().enumerate() {
        rating[i] = sqlx::query_scalar("SELECT rating FROM clans WHERE id = ?")
            .bind(id)
            .fetch_optional(&mut *conn)
            .await?
            .unwrap_or(1000);
    }
    let score = |t: usize| match winner_team {
        Some(w) if w as usize == t => 1.0,
        Some(_) => 0.0,
        None => 0.5,
    };
    let delta = |mine: i64, theirs: i64, s: f64| {
        let expected = 1.0 / (1.0 + 10f64.powf((theirs - mine) as f64 / 400.0));
        (CLAN_ELO_K * (s - expected)).round() as i64
    };
    let after = [
        rating[0] + delta(rating[0], rating[1], score(0)),
        rating[1] + delta(rating[1], rating[0], score(1)),
    ];
    for t in 0..2 {
        let (w, l, d) = match winner_team {
            Some(x) if x as usize == t => (1, 0, 0),
            Some(_) => (0, 1, 0),
            None => (0, 0, 1),
        };
        sqlx::query(
            "UPDATE clans SET rating = ?, war_wins = war_wins + ?, war_losses = war_losses + ?,
                war_draws = war_draws + ? WHERE id = ?",
        )
        .bind(after[t])
        .bind(w)
        .bind(l)
        .bind(d)
        .bind(war.clan_ids[t])
        .execute(&mut *conn)
        .await?;
    }
    if let Some(w) = winner_team {
        seasons::add_clan_points(
            conn,
            season,
            war.clan_ids[w as usize],
            CLAN_POINTS_WAR_WIN,
            true,
        )
        .await?;
    }
    sqlx::query(
        "UPDATE clan_wars SET status = 'done', updated_at = ?, match_id = ?, winner_id = ?,
            challenger_before = ?, challenger_after = ?, defender_before = ?, defender_after = ?
         WHERE id = ?",
    )
    .bind(now())
    .bind(match_id)
    .bind(winner_team.map(|w| war.clan_ids[w as usize]))
    .bind(rating[0])
    .bind(after[0])
    .bind(rating[1])
    .bind(after[1])
    .bind(war.id)
    .execute(&mut *conn)
    .await?;
    let tournament: Option<i64> =
        sqlx::query_scalar("SELECT tournament_id FROM clan_wars WHERE id = ?")
            .bind(war.id)
            .fetch_one(&mut *conn)
            .await?;
    if let Some(tid) = tournament {
        let winner = winner_team.map(|w| war.clan_ids[w as usize]);
        crate::tournaments::on_war_done(conn, tid, war.id, winner, war.clan_ids, rating).await?;
    }
    Ok(())
}

/// Le guerre rimaste "in corso" dopo un riavvio tornano accettate.
pub async fn recover(db: &SqlitePool) -> sqlx::Result<()> {
    sqlx::query("UPDATE clan_wars SET status = 'accepted' WHERE status = 'live'")
        .execute(db)
        .await?;
    Ok(())
}

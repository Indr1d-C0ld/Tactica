//! Tornei fra clan a eliminazione diretta.
//!
//! - Un capo o ufficiale crea il torneo (il suo clan è iscritto) e altri clan
//!   si iscrivono fino al massimo previsto.
//! - All'avvio i clan vengono ordinati per ELO di clan (teste di serie) in un
//!   tabellone a potenza di due; i posti mancanti diventano turni liberi.
//! - Ogni incontro è una guerra fra clan già accettata: si gioca con gli
//!   schieramenti normali. Quando una guerra finisce il vincitore avanza;
//!   in caso di pareggio passa il clan con ELO più alto.
//! - Alla finale: titoli e monete ai membri di campioni e finalisti, punti
//!   stagionali al clan campione.

#![allow(clippy::type_complexity)]

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;

use crate::AppState;
use crate::auth::AuthUser;
use crate::clans::{self, Membership};
use crate::db::now;
use crate::error::{ApiError, ApiResult};
use crate::seasons;

pub const MAX_CLANS: i64 = 16;
pub const CHAMPION_COINS: i64 = 500;
pub const FINALIST_COINS: i64 = 200;
pub const CHAMPION_POINTS: i64 = 15;

fn forbidden(m: &str) -> ApiError {
    ApiError::new(StatusCode::FORBIDDEN, m)
}

async fn captain(s: &AppState, user: &AuthUser) -> ApiResult<Membership> {
    let mut conn = s.db.acquire().await?;
    let m = clans::membership(&mut conn, user.id)
        .await?
        .ok_or_else(|| ApiError::conflict("non fai parte di un clan"))?;
    if m.role == "member" {
        return Err(forbidden("solo capo e ufficiali gestiscono i tornei"));
    }
    Ok(m)
}

/// Ordine standard delle teste di serie nel tabellone (1 contro n, ecc.).
pub fn seed_order(size: usize) -> Vec<usize> {
    let mut order = vec![1];
    while order.len() < size {
        let n = order.len() * 2;
        order = order.iter().flat_map(|&s| [s, n + 1 - s]).collect();
    }
    order
}

// ---------------------------------------------------------------- consultazione

#[derive(Serialize)]
pub struct Summary {
    id: i64,
    name: String,
    status: String,
    clans: i64,
    max_clans: i64,
    winner: Option<String>,
    created_at: i64,
}

pub async fn list(State(s): State<AppState>) -> ApiResult<Json<Vec<Summary>>> {
    let rows = sqlx::query_as::<_, (i64, String, String, i64, i64, Option<String>, i64)>(
        "SELECT t.id, t.name, t.status,
                (SELECT COUNT(*) FROM tournament_entries e WHERE e.tournament_id = t.id),
                t.max_clans, w.tag, t.created_at
         FROM tournaments t LEFT JOIN clans w ON w.id = t.winner_clan
         ORDER BY CASE t.status WHEN 'running' THEN 0 WHEN 'signup' THEN 1 ELSE 2 END, t.id DESC LIMIT 50",
    )
    .fetch_all(&s.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(
                |(id, name, status, clans, max_clans, winner, created_at)| Summary {
                    id,
                    name,
                    status,
                    clans,
                    max_clans,
                    winner,
                    created_at,
                },
            )
            .collect(),
    ))
}

#[derive(Serialize)]
pub struct Entry {
    tag: String,
    name: String,
    rating: i64,
    seed: Option<i64>,
}

#[derive(Serialize)]
pub struct BracketMatch {
    round: i64,
    slot: i64,
    a: Option<String>,
    b: Option<String>,
    war_id: Option<i64>,
    war_status: Option<String>,
    winner: Option<String>,
    bye: bool,
    /// Scadenza per giocare l'incontro (secondi Unix).
    deadline: Option<i64>,
    /// "forfeit" (a tavolino per scadenza) o "admin"; assente se giocato.
    decided_by: Option<String>,
}

#[derive(Serialize)]
pub struct Detail {
    id: i64,
    name: String,
    status: String,
    max_clans: i64,
    creator: Option<String>,
    winner: Option<String>,
    rounds: i64,
    entries: Vec<Entry>,
    matches: Vec<BracketMatch>,
}

pub async fn detail(State(s): State<AppState>, Path(id): Path<i64>) -> ApiResult<Json<Detail>> {
    let (tid, name, status, max_clans, creator, winner) = sqlx::query_as::<_, (i64, String, String, i64, Option<String>, Option<String>)>(
        "SELECT t.id, t.name, t.status, t.max_clans, c.tag, w.tag FROM tournaments t
         LEFT JOIN clans c ON c.id = t.creator_clan LEFT JOIN clans w ON w.id = t.winner_clan WHERE t.id = ?",
    )
    .bind(id)
    .fetch_optional(&s.db)
    .await?
    .ok_or_else(|| ApiError::not_found("torneo non trovato"))?;
    let entries = sqlx::query_as::<_, (String, String, i64, Option<i64>)>(
        "SELECT c.tag, c.name, c.rating, e.seed FROM tournament_entries e JOIN clans c ON c.id = e.clan_id
         WHERE e.tournament_id = ? ORDER BY COALESCE(e.seed, 1000), c.rating DESC",
    )
    .bind(id)
    .fetch_all(&s.db)
    .await?
    .into_iter()
    .map(|(tag, name, rating, seed)| Entry { tag, name, rating, seed })
    .collect();
    #[allow(clippy::type_complexity)]
    let matches: Vec<BracketMatch> = sqlx::query_as::<_, (i64, i64, Option<String>, Option<String>, Option<i64>, Option<String>, Option<String>, bool, Option<i64>, Option<String>)>(
        "SELECT m.round, m.slot, a.tag, b.tag, m.war_id, w.status, x.tag, m.clan_b IS NULL AND m.clan_a IS NOT NULL,
                m.deadline, m.decided_by
         FROM tournament_matches m
         LEFT JOIN clans a ON a.id = m.clan_a LEFT JOIN clans b ON b.id = m.clan_b
         LEFT JOIN clans x ON x.id = m.winner_clan LEFT JOIN clan_wars w ON w.id = m.war_id
         WHERE m.tournament_id = ? ORDER BY m.round, m.slot",
    )
    .bind(id)
    .fetch_all(&s.db)
    .await?
    .into_iter()
    .map(
        |(round, slot, a, b, war_id, war_status, winner, bye, deadline, decided_by)| BracketMatch {
            round,
            slot,
            a,
            b,
            war_id,
            war_status,
            winner,
            bye,
            deadline,
            decided_by,
        },
    )
    .collect();
    let rounds = matches.iter().map(|m| m.round).max().unwrap_or(0);
    let size: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tournament_entries WHERE tournament_id = ?")
            .bind(id)
            .fetch_one(&s.db)
            .await?;
    let total_rounds = if status == "signup" {
        0
    } else {
        let mut r = 0;
        while (1i64 << r) < size {
            r += 1;
        }
        r.max(rounds)
    };
    Ok(Json(Detail {
        id: tid,
        name,
        status,
        max_clans,
        creator,
        winner,
        rounds: total_rounds,
        entries,
        matches,
    }))
}

// ---------------------------------------------------------------- iscrizioni

#[derive(Deserialize)]
pub struct Create {
    name: String,
    #[serde(default = "default_max")]
    max_clans: i64,
}

fn default_max() -> i64 {
    8
}

pub async fn create(
    State(s): State<AppState>,
    user: AuthUser,
    Json(c): Json<Create>,
) -> ApiResult<Json<Detail>> {
    let me = captain(&s, &user).await?;
    let name = c.name.trim().to_string();
    if !(3..=40).contains(&name.chars().count()) || name.chars().any(char::is_control) {
        return Err(ApiError::bad_request(
            "nome del torneo: da 3 a 40 caratteri",
        ));
    }
    if !(2..=MAX_CLANS).contains(&c.max_clans) {
        return Err(ApiError::bad_request(format!("da 2 a {MAX_CLANS} clan")));
    }
    let mut tx = s.db.begin().await?;
    let id = sqlx::query(
        "INSERT INTO tournaments (name, creator_clan, max_clans, status, created_at, updated_at, match_hours)
         VALUES (?, ?, ?, 'signup', ?, ?, ?)",
    )
    .bind(&name)
    .bind(me.clan_id)
    .bind(c.max_clans)
    .bind(now())
    .bind(now())
    .bind(s.config.tournament_match_hours)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();
    sqlx::query(
        "INSERT INTO tournament_entries (tournament_id, clan_id, joined_at) VALUES (?, ?, ?)",
    )
    .bind(id)
    .bind(me.clan_id)
    .bind(now())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    tracing::info!(tournament = id, %name, "torneo creato");
    detail(State(s), Path(id)).await
}

async fn status_of(conn: &mut SqliteConnection, id: i64) -> ApiResult<(String, i64, Option<i64>)> {
    sqlx::query_as("SELECT status, max_clans, creator_clan FROM tournaments WHERE id = ?")
        .bind(id)
        .fetch_optional(conn)
        .await?
        .ok_or_else(|| ApiError::not_found("torneo non trovato"))
}

pub async fn join(
    State(s): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Detail>> {
    let me = captain(&s, &user).await?;
    let mut tx = s.db.begin().await?;
    let (status, max, _) = status_of(&mut tx, id).await?;
    if status != "signup" {
        return Err(ApiError::conflict("le iscrizioni sono chiuse"));
    }
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tournament_entries WHERE tournament_id = ?")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
    if count >= max {
        return Err(ApiError::conflict("il torneo è al completo"));
    }
    let n = sqlx::query("INSERT OR IGNORE INTO tournament_entries (tournament_id, clan_id, joined_at) VALUES (?, ?, ?)")
        .bind(id)
        .bind(me.clan_id)
        .bind(now())
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(ApiError::conflict("il tuo clan è già iscritto"));
    }
    tx.commit().await?;
    detail(State(s), Path(id)).await
}

pub async fn leave(
    State(s): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Detail>> {
    let me = captain(&s, &user).await?;
    let mut conn = s.db.acquire().await?;
    let (status, _, creator) = status_of(&mut conn, id).await?;
    if status != "signup" {
        return Err(ApiError::conflict("il torneo è già iniziato"));
    }
    if creator == Some(me.clan_id) {
        return Err(ApiError::conflict(
            "il clan organizzatore può solo annullare il torneo",
        ));
    }
    sqlx::query("DELETE FROM tournament_entries WHERE tournament_id = ? AND clan_id = ?")
        .bind(id)
        .bind(me.clan_id)
        .execute(&mut *conn)
        .await?;
    drop(conn);
    detail(State(s), Path(id)).await
}

pub async fn cancel(
    State(s): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Detail>> {
    let me = captain(&s, &user).await?;
    let mut conn = s.db.acquire().await?;
    let (status, _, creator) = status_of(&mut conn, id).await?;
    if creator != Some(me.clan_id) {
        return Err(forbidden("solo il clan organizzatore può annullare"));
    }
    if status != "signup" {
        return Err(ApiError::conflict("il torneo è già iniziato"));
    }
    sqlx::query("UPDATE tournaments SET status = 'cancelled', updated_at = ? WHERE id = ?")
        .bind(now())
        .bind(id)
        .execute(&mut *conn)
        .await?;
    drop(conn);
    detail(State(s), Path(id)).await
}

/// Avvio: teste di serie per ELO di clan, primo turno con eventuali turni liberi.
pub async fn start(
    State(s): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Detail>> {
    let me = captain(&s, &user).await?;
    let mut tx = s.db.begin().await?;
    let (status, _, creator) = status_of(&mut tx, id).await?;
    if creator != Some(me.clan_id) {
        return Err(forbidden(
            "solo il clan organizzatore può avviare il torneo",
        ));
    }
    if status != "signup" {
        return Err(ApiError::conflict("il torneo è già iniziato"));
    }
    let clans: Vec<(i64,)> = sqlx::query_as(
        "SELECT e.clan_id FROM tournament_entries e JOIN clans c ON c.id = e.clan_id
         WHERE e.tournament_id = ? ORDER BY c.rating DESC, e.joined_at",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    if clans.len() < 2 {
        return Err(ApiError::conflict("servono almeno 2 clan"));
    }
    for (i, (clan,)) in clans.iter().enumerate() {
        sqlx::query(
            "UPDATE tournament_entries SET seed = ? WHERE tournament_id = ? AND clan_id = ?",
        )
        .bind(i as i64 + 1)
        .bind(id)
        .bind(clan)
        .execute(&mut *tx)
        .await?;
    }
    let size = clans.len().next_power_of_two();
    let order = seed_order(size);
    for (slot, pair) in order.chunks(2).enumerate() {
        let a = clans.get(pair[0] - 1).map(|c| c.0);
        let b = clans.get(pair[1] - 1).map(|c| c.0);
        create_match(&mut tx, id, 1, slot as i64, a, b).await?;
    }
    sqlx::query("UPDATE tournaments SET status = 'running', updated_at = ? WHERE id = ?")
        .bind(now())
        .bind(id)
        .execute(&mut *tx)
        .await?;
    // I turni liberi fanno già avanzare qualcuno.
    advance(&mut tx, id).await?;
    tx.commit().await?;
    tracing::info!(tournament = id, clans = clans.len(), "torneo iniziato");
    detail(State(s), Path(id)).await
}

/// Crea un incontro: con due clan apre una guerra già accettata; con uno solo
/// è un turno libero e il clan avanza subito.
async fn create_match(
    conn: &mut SqliteConnection,
    tid: i64,
    round: i64,
    slot: i64,
    a: Option<i64>,
    b: Option<i64>,
) -> sqlx::Result<()> {
    let (war, winner) = match (a, b) {
        (Some(a), Some(b)) => {
            let w = sqlx::query(
                "INSERT INTO clan_wars (challenger_id, defender_id, status, created_at, updated_at, tournament_id)
                 VALUES (?, ?, 'accepted', ?, ?, ?)",
            )
            .bind(a)
            .bind(b)
            .bind(now())
            .bind(now())
            .bind(tid)
            .execute(&mut *conn)
            .await?
            .last_insert_rowid();
            (Some(w), None)
        }
        (Some(a), None) => (None, Some(a)),
        (None, Some(b)) => (None, Some(b)),
        (None, None) => (None, None),
    };
    let hours: i64 = sqlx::query_scalar("SELECT match_hours FROM tournaments WHERE id = ?")
        .bind(tid)
        .fetch_one(&mut *conn)
        .await?;
    let deadline = war.map(|_| now() + hours * 3600);
    sqlx::query(
        "INSERT INTO tournament_matches (tournament_id, round, slot, clan_a, clan_b, war_id, winner_clan, deadline)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(tid)
    .bind(round)
    .bind(slot)
    .bind(a)
    .bind(b)
    .bind(war)
    .bind(winner)
    .bind(deadline)
    .execute(conn)
    .await?;
    Ok(())
}

/// Chiamata da `wars::record_result` (stessa transazione) per le guerre di torneo.
pub async fn on_war_done(
    conn: &mut SqliteConnection,
    tid: i64,
    war: i64,
    winner: Option<i64>,
    clans: [i64; 2],
    ratings_before: [i64; 2],
) -> sqlx::Result<()> {
    // Torneo annullato nel frattempo (da un admin): la guerra conta, il tabellone no.
    let status: Option<String> = sqlx::query_scalar("SELECT status FROM tournaments WHERE id = ?")
        .bind(tid)
        .fetch_optional(&mut *conn)
        .await?;
    if status.as_deref() != Some("running") {
        return Ok(());
    }
    // Pareggio: passa il clan con ELO più alto prima della guerra (a parità, lo sfidante).
    let advancing = winner.unwrap_or(if ratings_before[1] > ratings_before[0] {
        clans[1]
    } else {
        clans[0]
    });
    sqlx::query(
        "UPDATE tournament_matches SET winner_clan = ? WHERE tournament_id = ? AND war_id = ?",
    )
    .bind(advancing)
    .bind(tid)
    .bind(war)
    .execute(&mut *conn)
    .await?;
    advance(conn, tid).await
}

/// Crea gli incontri del turno successivo quando entrambi gli incontri che li
/// alimentano hanno un vincitore; alla finale chiude il torneo e assegna i premi.
async fn advance(conn: &mut SqliteConnection, tid: i64) -> sqlx::Result<()> {
    loop {
        let rows: Vec<(i64, i64, Option<i64>, Option<i64>)> = sqlx::query_as(
            "SELECT round, slot, winner_clan, clan_b FROM tournament_matches WHERE tournament_id = ? ORDER BY round, slot",
        )
        .bind(tid)
        .fetch_all(&mut *conn)
        .await?;
        let last_round = rows.iter().map(|r| r.0).max().unwrap_or(0);
        let in_last: Vec<&(i64, i64, Option<i64>, Option<i64>)> =
            rows.iter().filter(|r| r.0 == last_round).collect();
        if in_last.len() == 1 {
            // Finale (o torneo a due): chiusura quando c'è il vincitore.
            if let Some(champion) = in_last[0].2 {
                finish(conn, tid, last_round, champion).await?;
            }
            return Ok(());
        }
        if in_last.iter().any(|r| r.2.is_none()) {
            return Ok(());
        }
        // Tutti i vincitori del turno: si accoppiano in ordine di slot.
        for pair in in_last.chunks(2) {
            let slot = pair[0].1 / 2;
            create_match(
                conn,
                tid,
                last_round + 1,
                slot,
                pair[0].2,
                pair.get(1).and_then(|p| p.2),
            )
            .await?;
        }
    }
}

async fn finish(
    conn: &mut SqliteConnection,
    tid: i64,
    final_round: i64,
    champion: i64,
) -> sqlx::Result<()> {
    let done: Option<String> = sqlx::query_scalar("SELECT status FROM tournaments WHERE id = ?")
        .bind(tid)
        .fetch_optional(&mut *conn)
        .await?;
    if done.as_deref() == Some("done") {
        return Ok(());
    }
    let name: String = sqlx::query_scalar("SELECT name FROM tournaments WHERE id = ?")
        .bind(tid)
        .fetch_one(&mut *conn)
        .await?;
    let (a, b): (Option<i64>, Option<i64>) = sqlx::query_as(
        "SELECT clan_a, clan_b FROM tournament_matches WHERE tournament_id = ? AND round = ?",
    )
    .bind(tid)
    .bind(final_round)
    .fetch_one(&mut *conn)
    .await?;
    let runner_up = [a, b].into_iter().flatten().find(|&c| c != champion);
    sqlx::query(
        "UPDATE tournaments SET status = 'done', winner_clan = ?, updated_at = ? WHERE id = ?",
    )
    .bind(champion)
    .bind(now())
    .bind(tid)
    .execute(&mut *conn)
    .await?;
    award_members(
        conn,
        champion,
        &format!("Campioni del torneo «{name}»"),
        CHAMPION_COINS,
    )
    .await?;
    achievements_for(conn, champion, &["tournament_final", "tournament_won"]).await?;
    if let Some(r) = runner_up {
        award_members(
            conn,
            r,
            &format!("Finalisti del torneo «{name}»"),
            FINALIST_COINS,
        )
        .await?;
        achievements_for(conn, r, &["tournament_final"]).await?;
    }
    let season = seasons::current(conn, 28).await?;
    seasons::add_clan_points(conn, &season, champion, CHAMPION_POINTS, true).await?;
    tracing::info!(tournament = tid, champion, "torneo concluso");
    Ok(())
}

/// Traguardi di torneo per tutti i membri di un clan.
async fn achievements_for(
    conn: &mut SqliteConnection,
    clan: i64,
    keys: &[&str],
) -> sqlx::Result<()> {
    let members: Vec<i64> =
        sqlx::query_scalar("SELECT account_id FROM clan_members WHERE clan_id = ?")
            .bind(clan)
            .fetch_all(&mut *conn)
            .await?;
    for acc in members {
        for k in keys {
            crate::achievements::peak(conn, acc, k, 1).await?;
        }
        crate::achievements::check(conn, acc).await?;
    }
    Ok(())
}

async fn award_members(
    conn: &mut SqliteConnection,
    clan: i64,
    title: &str,
    coins: i64,
) -> sqlx::Result<()> {
    let members: Vec<(i64,)> =
        sqlx::query_as("SELECT account_id FROM clan_members WHERE clan_id = ?")
            .bind(clan)
            .fetch_all(&mut *conn)
            .await?;
    for (acc,) in members {
        sqlx::query("INSERT OR IGNORE INTO titles (account_id, title, season_id, awarded_at) VALUES (?, ?, NULL, ?)")
            .bind(acc)
            .bind(title)
            .bind(now())
            .execute(&mut *conn)
            .await?;
        sqlx::query("UPDATE accounts SET coins = coins + ? WHERE id = ?")
            .bind(coins)
            .bind(acc)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

// ---------------------------------------------------------------- scadenze e decisioni

/// Assegna un incontro non ancora deciso: chiude la guerra (senza partita) e
/// fa avanzare il vincitore. `how` = "forfeit" o "admin".
pub async fn decide(
    conn: &mut SqliteConnection,
    tid: i64,
    round: i64,
    slot: i64,
    winner: i64,
    how: &str,
) -> sqlx::Result<bool> {
    let row: Option<(Option<i64>, Option<i64>, Option<i64>, Option<i64>)> = sqlx::query_as(
        "SELECT clan_a, clan_b, war_id, winner_clan FROM tournament_matches
         WHERE tournament_id = ? AND round = ? AND slot = ?",
    )
    .bind(tid)
    .bind(round)
    .bind(slot)
    .fetch_optional(&mut *conn)
    .await?;
    let Some((a, b, war, done)) = row else {
        return Ok(false);
    };
    if done.is_some() || (Some(winner) != a && Some(winner) != b) {
        return Ok(false);
    }
    if let Some(w) = war {
        sqlx::query(
            "UPDATE clan_wars SET status = 'cancelled', updated_at = ? WHERE id = ? AND status IN ('pending', 'accepted')",
        )
        .bind(now())
        .bind(w)
        .execute(&mut *conn)
        .await?;
    }
    sqlx::query(
        "UPDATE tournament_matches SET winner_clan = ?, decided_by = ?
         WHERE tournament_id = ? AND round = ? AND slot = ?",
    )
    .bind(winner)
    .bind(how)
    .bind(tid)
    .bind(round)
    .bind(slot)
    .execute(&mut *conn)
    .await?;
    advance(conn, tid).await?;
    Ok(true)
}

/// Incontri scaduti: vince il clan che si è presentato allo schieramento; se
/// non c'è nessuno o ci sono entrambi, passa il clan con l'ELO più alto (a
/// parità, il primo). Una guerra in corso non scade; con entrambi i clan
/// schierati la scadenza slitta di un'ora. Restituisce gli incontri decisi.
pub async fn expire_due(s: &AppState) -> anyhow::Result<usize> {
    let t = now();
    // Incontri creati prima delle scadenze: ne ricevono una da adesso.
    sqlx::query(
        "UPDATE tournament_matches SET deadline = ? + 3600 * (SELECT match_hours FROM tournaments t WHERE t.id = tournament_id)
         WHERE deadline IS NULL AND winner_clan IS NULL AND war_id IS NOT NULL",
    )
    .bind(t)
    .execute(&s.db)
    .await?;
    let due: Vec<(i64, i64, i64, i64, i64, i64, String, i64, i64)> = sqlx::query_as(
        "SELECT m.tournament_id, m.round, m.slot, m.clan_a, m.clan_b, m.war_id, w.status, ca.rating, cb.rating
         FROM tournament_matches m
         JOIN tournaments t ON t.id = m.tournament_id AND t.status = 'running'
         JOIN clan_wars w ON w.id = m.war_id
         JOIN clans ca ON ca.id = m.clan_a JOIN clans cb ON cb.id = m.clan_b
         WHERE m.winner_clan IS NULL AND m.deadline IS NOT NULL AND m.deadline <= ?",
    )
    .bind(t)
    .fetch_all(&s.db)
    .await?;
    let mut decided = 0;
    for (tid, round, slot, a, b, war, status, ra, rb) in due {
        if status == "live" {
            continue;
        }
        let present = {
            let lobby = s.lobby.lock().unwrap();
            lobby
                .war_room(war)
                .map(|r| [!r.sides[0].is_empty(), !r.sides[1].is_empty()])
                .unwrap_or([false, false])
        };
        if present == [true, true] {
            sqlx::query(
                "UPDATE tournament_matches SET deadline = ? WHERE tournament_id = ? AND round = ? AND slot = ?",
            )
            .bind(t + 3600)
            .bind(tid)
            .bind(round)
            .bind(slot)
            .execute(&s.db)
            .await?;
            continue;
        }
        let winner = match present {
            [true, false] => a,
            [false, true] => b,
            _ if rb > ra => b,
            _ => a,
        };
        let mut tx = s.db.begin().await?;
        if decide(&mut tx, tid, round, slot, winner, "forfeit").await? {
            decided += 1;
            tracing::info!(
                tournament = tid,
                round,
                slot,
                winner,
                "incontro di torneo assegnato a tavolino"
            );
        }
        tx.commit().await?;
        if let Some(room) = s.lobby.lock().unwrap().close_war_room(war) {
            for side in &room.sides {
                for m in side {
                    m.send(crate::protocol::ServerMsg::Error {
                        message: "incontro di torneo scaduto: assegnato a tavolino".into(),
                    });
                }
            }
        }
    }
    Ok(decided)
}

#[cfg(test)]
mod tests {
    use super::seed_order;

    #[test]
    fn standard_bracket_order() {
        assert_eq!(seed_order(2), vec![1, 2]);
        assert_eq!(seed_order(4), vec![1, 4, 2, 3]);
        assert_eq!(seed_order(8), vec![1, 8, 4, 5, 2, 7, 3, 6]);
    }
}

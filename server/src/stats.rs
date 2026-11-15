//! Persistenza delle partite e statistiche personali/globali.

use serde::Serialize;
use sqlx::SqlitePool;
use tactica_core::unit::Unit;
use tactica_core::{Action, Battle, BattleSetup};

use crate::db::{RosterUnit, now};
use crate::game::Seat;
use crate::protocol::{Mode, Reward};

pub const ELO_K: f64 = 32.0;
pub const XP_WIN_BONUS: i32 = 30;
pub const XP_PLAY_BONUS: i32 = 10;

pub struct SeatOutcome {
    pub result: &'static str,
    pub elo_before: i64,
    pub elo_after: i64,
    pub rewards: Vec<Reward>,
}

pub struct SavedMatch {
    pub match_id: i64,
    pub seats: [Option<SeatOutcome>; 2],
}

fn elo_delta(mine: i64, theirs: i64, score: f64) -> i64 {
    let expected = 1.0 / (1.0 + 10f64.powf((theirs - mine) as f64 / 400.0));
    (ELO_K * (score - expected)).round() as i64
}

#[allow(clippy::too_many_arguments)]
pub async fn save_match(
    db: &SqlitePool,
    mode: Mode,
    setup: &BattleSetup,
    actions: &[Action],
    battle: &Battle,
    seats: &[Seat; 2],
    roster_ids: &[Option<i64>],
    started: i64,
) -> anyhow::Result<SavedMatch> {
    let winner = battle.winner().flatten();
    let result = |team: u8| match winner {
        Some(w) if w == team => "win",
        Some(_) => "loss",
        None => "draw",
    };
    let mut tx = db.begin().await?;

    let match_id = sqlx::query(
        "INSERT INTO matches (mode, seed, setup, actions, finished, winner_team, turns, biome,
                              final_hash, started_at, ended_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(mode.key())
    .bind(setup.seed as i64)
    .bind(serde_json::to_string(setup)?)
    .bind(serde_json::to_string(actions)?)
    .bind(battle.is_over())
    .bind(winner.map(i64::from))
    .bind(battle.turn() as i64)
    .bind(battle.map.biome.name())
    .bind(format!("{:016x}", battle.state_hash()))
    .bind(started)
    .bind(now())
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();

    for s in &battle.statutes {
        sqlx::query("INSERT INTO match_statutes (match_id, statute) VALUES (?, ?)")
            .bind(match_id)
            .bind(s.name())
            .execute(&mut *tx)
            .await?;
    }

    // ELO solo in PvP tra due umani; i valori correnti si rileggono dal DB.
    let mut elo = [0i64; 2];
    for (team, seat) in seats.iter().enumerate() {
        if let Seat::Human(e) = seat {
            elo[team] = sqlx::query_scalar("SELECT elo FROM accounts WHERE id = ?")
                .bind(e.account_id)
                .fetch_one(&mut *tx)
                .await?;
        }
    }
    let ranked = mode == Mode::Pvp && seats.iter().all(|s| matches!(s, Seat::Human(_)));

    let mut out: [Option<SeatOutcome>; 2] = [None, None];
    for (team, seat) in seats.iter().enumerate() {
        let Seat::Human(e) = seat else { continue };
        let r = result(team as u8);
        let after = if ranked {
            let score = match r {
                "win" => 1.0,
                "loss" => 0.0,
                _ => 0.5,
            };
            elo[team] + elo_delta(elo[team], elo[1 - team], score)
        } else {
            elo[team]
        };
        sqlx::query("UPDATE accounts SET elo = ? WHERE id = ?")
            .bind(after)
            .bind(e.account_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO match_players (match_id, team, account_id, result, elo_before, elo_after)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(match_id)
        .bind(team as i64)
        .bind(e.account_id)
        .bind(r)
        .bind(elo[team])
        .bind(after)
        .execute(&mut *tx)
        .await?;
        out[team] = Some(SeatOutcome {
            result: r,
            elo_before: elo[team],
            elo_after: after,
            rewards: Vec::new(),
        });
    }

    for (i, u) in battle.units.iter().enumerate() {
        let account = match &seats[u.team as usize] {
            Seat::Human(e) => Some(e.account_id),
            Seat::Ai => None,
        };
        let roster_id = roster_ids.get(i).copied().flatten();
        let level_start = setup.units[i].level as i64;

        // Esperienza della battaglia + bonus di fine partita.
        let mut progressed: Unit = u.clone();
        let bonus = if winner == Some(u.team) {
            XP_WIN_BONUS
        } else {
            XP_PLAY_BONUS
        };
        progressed.gain_xp(bonus);

        sqlx::query(
            "INSERT INTO match_units (match_id, unit, team, account_id, roster_id, class,
                level_start, level_end, survived, damage_dealt, damage_taken, healing_done,
                kills, actions, misses, violations, xp_gained)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(match_id)
        .bind(i as i64)
        .bind(u.team as i64)
        .bind(account)
        .bind(roster_id)
        .bind(u.class.key())
        .bind(level_start)
        .bind(progressed.level as i64)
        .bind(u.is_active())
        .bind(u.tally.damage_dealt)
        .bind(u.tally.damage_taken)
        .bind(u.tally.healing_done)
        .bind(u.tally.kills)
        .bind(u.tally.actions)
        .bind(u.tally.misses)
        .bind(u.tally.violations)
        .bind(progressed.tally.xp_gained)
        .execute(&mut *tx)
        .await?;

        if let (Some(rid), Some(acc)) = (roster_id, account) {
            sqlx::query("UPDATE roster SET level = ?, xp = ? WHERE id = ? AND account_id = ?")
                .bind(progressed.level as i64)
                .bind(progressed.xp as i64)
                .bind(rid)
                .bind(acc)
                .execute(&mut *tx)
                .await?;
            if let Some(o) = out[u.team as usize].as_mut() {
                o.rewards.push(Reward {
                    roster_id: rid,
                    name: u.name.clone(),
                    level_before: level_start,
                    level_after: progressed.level as i64,
                    xp: progressed.tally.xp_gained as i64,
                });
            }
        }
    }

    tx.commit().await?;
    Ok(SavedMatch {
        match_id,
        seats: out,
    })
}

// ---------------------------------------------------------------- consultazione

#[derive(Serialize, Default)]
pub struct Totals {
    pub units_fielded: i64,
    pub damage_dealt: i64,
    pub damage_taken: i64,
    pub healing_done: i64,
    pub kills: i64,
    pub violations: i64,
}

#[derive(Serialize)]
pub struct ClassLine {
    pub class: String,
    pub fielded: i64,
    pub kills: i64,
    pub damage_dealt: i64,
    pub survival_rate: f64,
}

#[derive(Serialize)]
pub struct RecentMatch {
    pub id: i64,
    pub mode: String,
    pub result: String,
    pub biome: String,
    pub turns: i64,
    pub elo_after: i64,
    pub ended_at: i64,
}

#[derive(Serialize)]
pub struct Profile {
    pub username: String,
    pub elo: i64,
    pub created_at: i64,
    pub wins: i64,
    pub losses: i64,
    pub draws: i64,
    pub totals: Totals,
    pub classes: Vec<ClassLine>,
    pub roster: Vec<RosterUnit>,
    pub recent: Vec<RecentMatch>,
}

pub async fn profile(db: &SqlitePool, username: &str) -> sqlx::Result<Option<Profile>> {
    let Some((id, username, elo, created_at)) = sqlx::query_as::<_, (i64, String, i64, i64)>(
        "SELECT id, username, elo, created_at FROM accounts WHERE username = ?",
    )
    .bind(username)
    .fetch_optional(db)
    .await?
    else {
        return Ok(None);
    };

    let (wins, losses, draws): (i64, i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(result = 'win'), 0), COALESCE(SUM(result = 'loss'), 0),
                COALESCE(SUM(result = 'draw'), 0)
         FROM match_players WHERE account_id = ?",
    )
    .bind(id)
    .fetch_one(db)
    .await?;

    let t: (i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(damage_dealt), 0), COALESCE(SUM(damage_taken), 0),
                COALESCE(SUM(healing_done), 0), COALESCE(SUM(kills), 0), COALESCE(SUM(violations), 0)
         FROM match_units WHERE account_id = ?",
    )
    .bind(id)
    .fetch_one(db)
    .await?;

    let classes = sqlx::query_as::<_, (String, i64, i64, i64, f64)>(
        "SELECT class, COUNT(*), SUM(kills), SUM(damage_dealt), AVG(survived)
         FROM match_units WHERE account_id = ? GROUP BY class ORDER BY COUNT(*) DESC",
    )
    .bind(id)
    .fetch_all(db)
    .await?
    .into_iter()
    .map(
        |(class, fielded, kills, damage_dealt, survival_rate)| ClassLine {
            class,
            fielded,
            kills,
            damage_dealt,
            survival_rate,
        },
    )
    .collect();

    let recent = sqlx::query_as::<_, (i64, String, String, String, i64, i64, i64)>(
        "SELECT m.id, m.mode, mp.result, m.biome, m.turns, mp.elo_after, m.ended_at
         FROM match_players mp JOIN matches m ON m.id = mp.match_id
         WHERE mp.account_id = ? ORDER BY m.id DESC LIMIT 10",
    )
    .bind(id)
    .fetch_all(db)
    .await?
    .into_iter()
    .map(
        |(id, mode, result, biome, turns, elo_after, ended_at)| RecentMatch {
            id,
            mode,
            result,
            biome,
            turns,
            elo_after,
            ended_at,
        },
    )
    .collect();

    Ok(Some(Profile {
        username,
        elo,
        created_at,
        wins,
        losses,
        draws,
        totals: Totals {
            units_fielded: t.0,
            damage_dealt: t.1,
            damage_taken: t.2,
            healing_done: t.3,
            kills: t.4,
            violations: t.5,
        },
        classes,
        roster: crate::db::roster(db, id).await?,
        recent,
    }))
}

#[derive(Serialize)]
pub struct LeaderRow {
    pub username: String,
    pub elo: i64,
    pub wins: i64,
    pub losses: i64,
}

pub async fn leaderboard(db: &SqlitePool, limit: i64) -> sqlx::Result<Vec<LeaderRow>> {
    Ok(sqlx::query_as::<_, (String, i64, i64, i64)>(
        "SELECT a.username, a.elo,
                (SELECT COUNT(*) FROM match_players p JOIN matches m ON m.id = p.match_id
                  WHERE p.account_id = a.id AND m.mode = 'pvp' AND p.result = 'win'),
                (SELECT COUNT(*) FROM match_players p JOIN matches m ON m.id = p.match_id
                  WHERE p.account_id = a.id AND m.mode = 'pvp' AND p.result = 'loss')
         FROM accounts a ORDER BY a.elo DESC, a.id LIMIT ?",
    )
    .bind(limit)
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|(username, elo, wins, losses)| LeaderRow {
        username,
        elo,
        wins,
        losses,
    })
    .collect())
}

#[derive(Serialize)]
pub struct ClassGlobal {
    pub class: String,
    pub fielded: i64,
    pub win_rate: f64,
    pub avg_damage: f64,
    pub avg_healing: f64,
    pub avg_kills: f64,
}

#[derive(Serialize)]
pub struct BiomeGlobal {
    pub biome: String,
    pub matches: i64,
    pub avg_turns: f64,
}

#[derive(Serialize)]
pub struct StatuteGlobal {
    pub statute: String,
    pub matches: i64,
    pub violations_per_match: f64,
}

#[derive(Serialize)]
pub struct Global {
    pub accounts: i64,
    pub matches: i64,
    pub pvp_matches: i64,
    pub draws: i64,
    pub classes: Vec<ClassGlobal>,
    pub biomes: Vec<BiomeGlobal>,
    pub statutes: Vec<StatuteGlobal>,
}

pub async fn global(db: &SqlitePool) -> sqlx::Result<Global> {
    let accounts = sqlx::query_scalar("SELECT COUNT(*) FROM accounts")
        .fetch_one(db)
        .await?;
    let (matches, pvp_matches, draws): (i64, i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(mode = 'pvp'), 0), COALESCE(SUM(winner_team IS NULL), 0)
         FROM matches",
    )
    .fetch_one(db)
    .await?;

    let classes = sqlx::query_as::<_, (String, i64, f64, f64, f64, f64)>(
        "SELECT mu.class, COUNT(*), AVG(m.winner_team IS NOT NULL AND m.winner_team = mu.team),
                AVG(mu.damage_dealt), AVG(mu.healing_done), AVG(mu.kills)
         FROM match_units mu JOIN matches m ON m.id = mu.match_id
         GROUP BY mu.class ORDER BY mu.class",
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(
        |(class, fielded, win_rate, avg_damage, avg_healing, avg_kills)| ClassGlobal {
            class,
            fielded,
            win_rate,
            avg_damage,
            avg_healing,
            avg_kills,
        },
    )
    .collect();

    let biomes = sqlx::query_as::<_, (String, i64, f64)>(
        "SELECT biome, COUNT(*), AVG(turns) FROM matches GROUP BY biome ORDER BY biome",
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|(biome, matches, avg_turns)| BiomeGlobal {
        biome,
        matches,
        avg_turns,
    })
    .collect();

    let statutes = sqlx::query_as::<_, (String, i64, f64)>(
        "SELECT ms.statute, COUNT(*),
                AVG((SELECT COALESCE(SUM(violations), 0) FROM match_units mu WHERE mu.match_id = ms.match_id))
         FROM match_statutes ms GROUP BY ms.statute ORDER BY ms.statute",
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|(statute, matches, violations_per_match)| StatuteGlobal {
        statute,
        matches,
        violations_per_match,
    })
    .collect();

    Ok(Global {
        accounts,
        matches,
        pvp_matches,
        draws,
        classes,
        biomes,
        statutes,
    })
}

#[derive(Serialize)]
pub struct MatchDetail {
    pub id: i64,
    pub mode: String,
    pub setup: serde_json::Value,
    pub actions: serde_json::Value,
    pub finished: bool,
    pub winner_team: Option<i64>,
    pub turns: i64,
    pub biome: String,
    pub final_hash: String,
    pub started_at: i64,
    pub ended_at: i64,
}

pub async fn match_detail(db: &SqlitePool, id: i64) -> anyhow::Result<Option<MatchDetail>> {
    let row = sqlx::query_as::<
        _,
        (
            i64,
            String,
            String,
            String,
            bool,
            Option<i64>,
            i64,
            String,
            String,
            i64,
            i64,
        ),
    >(
        "SELECT id, mode, setup, actions, finished, winner_team, turns, biome, final_hash,
                started_at, ended_at FROM matches WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(db)
    .await?;
    let Some(r) = row else { return Ok(None) };
    Ok(Some(MatchDetail {
        id: r.0,
        mode: r.1,
        setup: serde_json::from_str(&r.2)?,
        actions: serde_json::from_str(&r.3)?,
        finished: r.4,
        winner_team: r.5,
        turns: r.6,
        biome: r.7,
        final_hash: r.8,
        started_at: r.9,
        ended_at: r.10,
    }))
}

//! Persistenza delle partite e statistiche personali/globali.

use std::collections::HashSet;

use serde::Serialize;
use sqlx::SqlitePool;
use tactica_core::gear::ItemRef;
use tactica_core::progression::{self, MatchResult};
use tactica_core::unit::Unit;
use tactica_core::{Action, Battle, BattleSetup};

use crate::db::{self, RosterUnit, now};
use crate::game::{MissionInfo, Player};
use crate::protocol::{Mode, Reward, RpGain};
use crate::wars::{self, WarInfo};
use crate::{clans, seasons};

pub const ELO_K: f64 = 32.0;
pub const XP_WIN_BONUS: i32 = 30;
pub const XP_PLAY_BONUS: i32 = 10;

pub struct SeatOutcome {
    pub result: &'static str,
    pub elo_before: i64,
    pub elo_after: i64,
    pub coins: i64,
    /// Oggetti raccolti in battaglia dalle unità del giocatore.
    pub loot: Vec<ItemRef>,
    pub season_before: Option<i64>,
    pub season_after: Option<i64>,
    pub clan_points: i64,
    pub rewards: Vec<Reward>,
}

pub struct SavedMatch {
    pub match_id: i64,
    /// Esito per giocatore (stesso ordine dei `players`; `None` per l'IA).
    pub players: Vec<Option<SeatOutcome>>,
}

/// Monete per una missione cooperativa vinta, secondo la difficoltà.
pub fn coop_win_coins(difficulty: u8) -> i64 {
    150 + 50 * (difficulty.max(1) as i64 - 1)
}

fn match_result(r: &str) -> MatchResult {
    match r {
        "win" => MatchResult::Win,
        "loss" => MatchResult::Loss,
        _ => MatchResult::Draw,
    }
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
    players: &[Player],
    roster_ids: &[Option<i64>],
    started: i64,
    season_days: i64,
    mission: Option<&MissionInfo>,
    war: Option<&WarInfo>,
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
                              final_hash, started_at, ended_at, mission, rules_version)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
    .bind(mission.map(|m| m.name.clone()))
    .bind(tactica_core::RULES_VERSION as i64)
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

    // Classificata solo in PvP con un umano per parte.
    let humans: Vec<usize> = (0..players.len())
        .filter(|&i| players[i].human().is_some())
        .collect();
    let ranked = mode == Mode::Pvp
        && humans.len() == 2
        && players[humans[0]].team != players[humans[1]].team;
    let opponent = |i: usize| {
        humans
            .iter()
            .copied()
            .find(|&j| players[j].team != players[i].team)
    };

    let mut elo = vec![0i64; players.len()];
    for &i in &humans {
        elo[i] = sqlx::query_scalar("SELECT elo FROM accounts WHERE id = ?")
            .bind(players[i].human().unwrap().account_id)
            .fetch_one(&mut *tx)
            .await?;
    }
    let season = seasons::current(&mut tx, season_days).await?;
    let mut season_rating = vec![0i64; players.len()];
    if ranked {
        for &i in &humans {
            season_rating[i] =
                seasons::rating(&mut tx, &season, players[i].human().unwrap().account_id).await?;
        }
    }

    let mut out: Vec<Option<SeatOutcome>> = (0..players.len()).map(|_| None).collect();
    let mut clans_awarded: HashSet<i64> = HashSet::new();
    for &i in &humans {
        let e = players[i].human().unwrap();
        let team = players[i].team;
        let r = result(team);
        let mr = match_result(r);
        let clan = clans::membership(&mut tx, e.account_id).await?;
        let mut coins = match (mode, mr, mission) {
            (Mode::Coop | Mode::Campaign, MatchResult::Win, Some(m)) => {
                coop_win_coins(m.difficulty)
            }
            (Mode::War, MatchResult::Win, _) => wars::WAR_WIN_COINS,
            (Mode::War, MatchResult::Loss, _) => wars::WAR_LOSS_COINS,
            (Mode::War, MatchResult::Draw, _) => wars::WAR_DRAW_COINS,
            _ => progression::coins(mr),
        };
        if clan.is_some() {
            coins += coins * clans::COIN_BONUS_PCT / 100;
        }
        let score = match r {
            "win" => 1.0,
            "loss" => 0.0,
            _ => 0.5,
        };
        let (season_before, season_after, after) = match (ranked, opponent(i)) {
            (true, Some(o)) => {
                let before = season_rating[i];
                let s_after = before + elo_delta(before, season_rating[o], score);
                seasons::record(&mut tx, &season, e.account_id, s_after, r).await?;
                (
                    Some(before),
                    Some(s_after),
                    elo[i] + elo_delta(elo[i], elo[o], score),
                )
            }
            _ => (None, None, elo[i]),
        };
        let clan_points = match (mode, r) {
            (Mode::Pvp, "win") if ranked => seasons::CLAN_POINTS_PVP_WIN,
            (Mode::Pvp, "draw") if ranked => seasons::CLAN_POINTS_PVP_DRAW,
            (Mode::Pve | Mode::Campaign, "win") => seasons::CLAN_POINTS_PVE_WIN,
            (Mode::Coop, "win") => {
                seasons::CLAN_POINTS_COOP_WIN
                    + if mission.is_some_and(|m| m.hunt) {
                        1
                    } else {
                        0
                    }
            }
            (Mode::War, "win") => wars::CLAN_POINTS_WAR_WIN,
            _ => 0,
        };
        let clan_points = if clan.is_some() { clan_points } else { 0 };
        if let Some(c) = &clan {
            // In cooperativa il clan prende i punti una volta sola per partita.
            // In guerra i punti li assegna `wars::record_result`, una volta per clan.
            if mode != Mode::War && (mode != Mode::Coop || clans_awarded.insert(c.clan_id)) {
                seasons::add_clan_points(&mut tx, &season, c.clan_id, clan_points, r == "win")
                    .await?;
            }
        }
        // Bottino: solo ciò che le unità del giocatore hanno raccolto in battaglia,
        // qualunque sia l'esito (anche chi perde tiene ciò che ha raccolto).
        let loot: Vec<ItemRef> = battle
            .picked
            .iter()
            .filter(|(u, _)| players[i].units.contains(&(*u as usize)))
            .map(|(_, item)| *item)
            .collect();
        sqlx::query("UPDATE accounts SET elo = ?, coins = coins + ? WHERE id = ?")
            .bind(after)
            .bind(coins)
            .bind(e.account_id)
            .execute(&mut *tx)
            .await?;
        for &item in &loot {
            db::add_item(&mut tx, e.account_id, item, 1).await?;
        }
        sqlx::query(
            "INSERT INTO match_players (match_id, team, account_id, result, elo_before, elo_after,
                                        coins, loot, season_id, clan_id, season_before, season_after)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(match_id)
        .bind(team as i64)
        .bind(e.account_id)
        .bind(r)
        .bind(elo[i])
        .bind(after)
        .bind(coins)
        .bind(
            (!loot.is_empty()).then(|| loot.iter().map(ItemRef::key).collect::<Vec<_>>().join(",")),
        )
        .bind(season.id)
        .bind(clan.as_ref().map(|c| c.clan_id))
        .bind(season_before)
        .bind(season_after)
        .execute(&mut *tx)
        .await?;
        out[i] = Some(SeatOutcome {
            result: r,
            elo_before: elo[i],
            elo_after: after,
            coins,
            loot,
            season_before,
            season_after,
            clan_points,
            rewards: Vec::new(),
        });
    }

    for (i, u) in battle.units.iter().enumerate() {
        let owner = players.iter().position(|p| p.units.contains(&i));
        let account = owner.and_then(|o| players[o].human()).map(|e| e.account_id);
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

        if let (Some(rid), Some(acc), Some(o)) = (roster_id, account, owner) {
            sqlx::query("UPDATE roster SET level = ?, xp = ? WHERE id = ? AND account_id = ?")
                .bind(progressed.level as i64)
                .bind(progressed.xp as i64)
                .bind(rid)
                .bind(acc)
                .execute(&mut *tx)
                .await?;
            // Risonanza: RP alle abilità insegnate dall'equipaggiamento schierato.
            let su = &setup.units[i];
            let gained = progression::rp_award(u, match_result(result(u.team)));
            let mut rp = Vec::new();
            for a in progression::training(su.class, &su.equipment, &su.mastered) {
                let current: i64 = sqlx::query_scalar(
                    "SELECT COALESCE((SELECT rp FROM roster_abilities WHERE roster_id = ? AND ability = ?), 0)",
                )
                .bind(rid)
                .bind(a.key())
                .fetch_one(&mut *tx)
                .await?;
                let (new_rp, mastered_now) = progression::add_rp(a, current as i32, gained);
                sqlx::query(
                    "INSERT INTO roster_abilities (roster_id, ability, rp, mastered) VALUES (?, ?, ?, ?)
                     ON CONFLICT(roster_id, ability) DO UPDATE SET rp = excluded.rp,
                         mastered = MAX(mastered, excluded.mastered)",
                )
                .bind(rid)
                .bind(a.key())
                .bind(new_rp as i64)
                .bind(new_rp >= a.def().master_rp)
                .execute(&mut *tx)
                .await?;
                rp.push(RpGain {
                    ability: a.key().to_string(),
                    name: a.def().name.to_string(),
                    gained: (new_rp as i64 - current).max(0),
                    rp: new_rp as i64,
                    master_rp: a.def().master_rp as i64,
                    mastered_now,
                });
            }
            if let Some(out) = out[o].as_mut() {
                out.rewards.push(Reward {
                    roster_id: rid,
                    name: u.name.clone(),
                    level_before: level_start,
                    level_after: progressed.level as i64,
                    xp: progressed.tally.xp_gained as i64,
                    rp,
                });
            }
        }
    }

    if let Some(w) = war {
        wars::record_result(&mut tx, w, winner, match_id, &season).await?;
    }
    if let (Mode::Campaign, Some(0), Some(node)) =
        (mode, winner, mission.and_then(|m| m.node.as_deref()))
    {
        for &i in &humans {
            let acc = players[i].human().unwrap().account_id;
            crate::campaign::record_clear(&mut tx, acc, node, battle.turn() as i64).await?;
        }
    }
    tx.commit().await?;
    Ok(SavedMatch {
        match_id,
        players: out,
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

pub async fn titles(db: &SqlitePool, account: i64) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar(
        "SELECT title FROM titles WHERE account_id = ? ORDER BY awarded_at DESC, title",
    )
    .bind(account)
    .fetch_all(db)
    .await
}

#[derive(Serialize)]
pub struct Profile {
    pub username: String,
    pub clan: Option<String>,
    pub titles: Vec<String>,
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

    let clan: Option<String> = sqlx::query_scalar(
        "SELECT c.tag FROM clan_members m JOIN clans c ON c.id = m.clan_id WHERE m.account_id = ?",
    )
    .bind(id)
    .fetch_optional(db)
    .await?;
    Ok(Some(Profile {
        clan,
        titles: titles(db, id).await?,
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
    /// Versione delle regole: il replay si riproduce solo con la stessa.
    pub rules_version: i64,
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
            i64,
        ),
    >(
        "SELECT id, mode, setup, actions, finished, winner_team, turns, biome, final_hash,
                started_at, ended_at, rules_version FROM matches WHERE id = ?",
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
        rules_version: r.11,
    }))
}

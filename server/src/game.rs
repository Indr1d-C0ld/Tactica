//! Esecuzione di una partita: un task per partita possiede la `Battle`,
//! riceve le intenzioni dei giocatori, le valida, fa giocare l'IA quando
//! serve e alla fine salva replay, statistiche, esperienza ed ELO.
//!
//! Ogni unità appartiene a un "giocatore" (umano o IA). Una squadra può avere
//! più giocatori umani (missioni cooperative): il server accetta un'azione solo
//! da chi controlla l'unità di turno.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tactica_core::presets;
use tactica_core::{Action, Battle, BattleSetup, Event, LootRules, Objective, UnitSetup, ai};
use tokio::sync::mpsc;
use tokio::time::{Instant, sleep, timeout_at};

use crate::AppState;
use crate::db::now;
use crate::lobby::{Entrant, ToConn};
use crate::protocol::{Mode, ServerMsg};
use crate::stats;
use crate::wars::{self, WarInfo};

pub struct MatchInput {
    pub seat: usize,
    pub kind: InputKind,
}

pub enum InputKind {
    Action(Action),
    /// Il giocatore abbandona: resa in PvE/PvP, passaggio all'IA in cooperativa.
    Leave,
    Disconnected,
}

pub enum Seat {
    Human(Entrant),
    Ai,
}

/// Chi controlla un gruppo di unità.
pub struct Player {
    pub entrant: Option<Entrant>,
    pub team: u8,
    pub units: Vec<usize>,
    pub connected: bool,
}

impl Player {
    pub fn human(&self) -> Option<&Entrant> {
        self.entrant.as_ref()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MissionInfo {
    pub name: String,
    pub difficulty: u8,
    pub difficulty_name: String,
    pub hunt: bool,
    /// Tappa della campagna (chiave), se è una battaglia di campagna.
    #[serde(default)]
    pub node: Option<String>,
    /// Prima battaglia guidata.
    #[serde(default)]
    pub tutorial: bool,
}

pub const STATUTES_PER_BATTLE: usize = 2;
/// Posti totali della squadra alleata in una missione.
pub const COOP_SLOTS: usize = 6;

/// Probabilità (%) che un'unità sconfitta lasci un oggetto, per modalità.
/// È l'unico modo di trovare bottino: bisogna raggiungere la casella dello sconfitto.
pub const LOOT_PVE: i32 = 45;
pub const LOOT_PVP: i32 = 35;
pub const LOOT_COOP: i32 = 55;
pub const LOOT_CAMPAIGN: i32 = 50;
pub const LOOT_WAR: i32 = 40;
const AI_NAMES: [&str; 4] = ["Sentinella", "Predone", "Veggente", "Eremita"];

fn random_seed() -> u64 {
    let mut b = [0u8; 8];
    getrandom::fill(&mut b).expect("sorgente casuale del sistema");
    u64::from_le_bytes(b)
}

/// PvE (umano contro IA che lo rispecchia) o PvP (due umani).
pub fn start_match(state: AppState, mode: Mode, seats: [Seat; 2]) {
    let mut units = Vec::new();
    let mut roster_ids = Vec::new();
    let mut players = Vec::new();
    for team in 0..2u8 {
        let start = units.len();
        match &seats[team as usize] {
            Seat::Human(e) => {
                for r in &e.units {
                    units.push(r.to_setup(team));
                    roster_ids.push(Some(r.id));
                }
            }
            Seat::Ai => {
                let Seat::Human(other) = &seats[1 - team as usize] else {
                    panic!("almeno un umano");
                };
                for (i, r) in other.units.iter().enumerate() {
                    let mut u = r.to_setup(team);
                    u.name = AI_NAMES[i % AI_NAMES.len()].to_string();
                    u.xp = 0;
                    units.push(u);
                    roster_ids.push(None);
                }
            }
        }
        let entrant = match &seats[team as usize] {
            Seat::Human(e) => Some(e.clone()),
            Seat::Ai => None,
        };
        players.push(Player {
            connected: entrant.is_some(),
            entrant,
            team,
            units: (start..units.len()).collect(),
        });
    }
    // PvE: lasciano oggetti i nemici dell'IA; PvP: entrambe le squadre.
    let loot = if mode == Mode::Pvp {
        LootRules {
            chance: [LOOT_PVP, LOOT_PVP],
            boost: [1, 1],
            champion: None,
        }
    } else {
        LootRules {
            chance: [0, LOOT_PVE],
            boost: [0, 0],
            champion: None,
        }
    };
    let setup = BattleSetup {
        seed: random_seed(),
        map: None,
        units,
        statutes: STATUTES_PER_BATTLE,
        objective: Objective::Rout,
        loot,
    };
    launch(state, mode, setup, players, roster_ids, None, None);
}

/// Missione cooperativa: i membri della stanza (stesso clan) contro l'IA.
pub fn start_mission(state: AppState, members: Vec<Entrant>, difficulty: u8) {
    let seed = random_seed();
    let quota = (COOP_SLOTS / members.len().max(1)).max(1);
    let mut allies: Vec<UnitSetup> = Vec::new();
    let mut roster_ids = Vec::new();
    let mut players = Vec::new();
    for e in members {
        let start = allies.len();
        let mut e = e;
        e.units.truncate(quota);
        for r in &e.units {
            allies.push(r.to_setup(0));
            roster_ids.push(Some(r.id));
        }
        players.push(Player {
            team: 0,
            units: (start..allies.len()).collect(),
            connected: true,
            entrant: Some(e),
        });
    }
    let mission = presets::mission(seed, &allies, difficulty);
    let enemy_start = allies.len();
    let objective = match mission.champion {
        Some(c) => Objective::Champion((enemy_start + c) as u32),
        None => Objective::Rout,
    };
    let mut units = allies;
    units.extend(mission.enemies.iter().cloned());
    roster_ids.extend(std::iter::repeat_n(None, mission.enemies.len()));
    players.push(Player {
        entrant: None,
        team: 1,
        units: (enemy_start..units.len()).collect(),
        connected: false,
    });
    let info = MissionInfo {
        name: mission.name.clone(),
        difficulty: mission.difficulty,
        difficulty_name: presets::difficulty_name(mission.difficulty).to_string(),
        hunt: mission.champion.is_some(),
        node: None,
        tutorial: false,
    };
    let loot = LootRules {
        chance: [0, LOOT_COOP],
        boost: [0, 2 * (difficulty as i32 - 1)],
        champion: mission.champion.map(|c| (enemy_start + c) as u32),
    };
    let setup = BattleSetup {
        seed,
        map: None,
        units,
        statutes: mission.statutes,
        objective,
        loot,
    };
    launch(
        state,
        Mode::Coop,
        setup,
        players,
        roster_ids,
        Some(info),
        None,
    );
}

/// Tappa della campagna: la squadra del giocatore contro nemici generati dal
/// seed, nel bioma della regione e con livelli crescenti lungo la storia.
pub fn start_campaign(state: AppState, entrant: Entrant, key: &str) {
    use tactica_core::MapParams;
    use tactica_core::rng::Rng;
    let Some((r, i)) = crate::campaign::find(key) else {
        return;
    };
    let region = &crate::campaign::REGIONS[r];
    let def = &region.nodes[i];
    let seed = random_seed();
    let mut allies: Vec<UnitSetup> = Vec::new();
    let mut roster_ids = Vec::new();
    for u in &entrant.units {
        allies.push(u.to_setup(0));
        roster_ids.push(Some(u.id));
    }
    let avg = allies.iter().map(|u| u.level).sum::<i32>() / allies.len().max(1) as i32;
    let mission = presets::mission(seed, &allies, def.difficulty);
    let mut enemies = mission.enemies.clone();
    if def.tutorial {
        enemies.truncate(2);
    }
    let base = (avg + def.level_bonus).clamp(1, tactica_core::class::MAX_LEVEL);
    for (k, e) in enemies.iter_mut().enumerate() {
        e.level = base;
        e.xp = 0;
        if def.champion && k == 0 {
            e.name = def.name.to_string();
            e.level = (base + 3).min(tactica_core::class::MAX_LEVEL);
        }
    }
    let enemy_start = allies.len();
    let objective = if def.champion {
        Objective::Champion(enemy_start as u32)
    } else {
        Objective::Rout
    };
    let mut units = allies;
    units.extend(enemies.iter().cloned());
    roster_ids.extend(std::iter::repeat_n(None, enemies.len()));
    let players = vec![
        Player {
            team: 0,
            units: (0..enemy_start).collect(),
            connected: true,
            entrant: Some(entrant),
        },
        Player {
            team: 1,
            units: (enemy_start..units.len()).collect(),
            connected: false,
            entrant: None,
        },
    ];
    let mut params = MapParams::random(&mut Rng::new(seed ^ 0x006d_6170));
    params.biome = region.biome;
    let info = MissionInfo {
        name: def.name.to_string(),
        difficulty: def.difficulty,
        difficulty_name: presets::difficulty_name(def.difficulty).to_string(),
        hunt: def.champion,
        node: Some(def.key.to_string()),
        tutorial: def.tutorial,
    };
    let loot = LootRules {
        chance: [0, if def.tutorial { 100 } else { LOOT_CAMPAIGN }],
        boost: [0, r as i32 + def.difficulty as i32 - 1],
        champion: def.champion.then_some(enemy_start as u32),
    };
    let setup = BattleSetup {
        seed,
        map: Some(params),
        units,
        statutes: if def.tutorial { 1 } else { mission.statutes },
        objective,
        loot,
    };
    launch(
        state,
        Mode::Campaign,
        setup,
        players,
        roster_ids,
        Some(info),
        None,
    );
}

/// Guerra fra clan: squadra 0 = sfidante, squadra 1 = sfidato; 6 posti per lato.
pub fn start_war(state: AppState, info: WarInfo, sides: [Vec<Entrant>; 2]) {
    let mut units: Vec<UnitSetup> = Vec::new();
    let mut roster_ids = Vec::new();
    let mut players = Vec::new();
    for (team, side) in sides.into_iter().enumerate() {
        let quota = (COOP_SLOTS / side.len().max(1)).max(1);
        for mut e in side {
            e.units.truncate(quota);
            let start = units.len();
            for r in &e.units {
                units.push(r.to_setup(team as u8));
                roster_ids.push(Some(r.id));
            }
            players.push(Player {
                team: team as u8,
                units: (start..units.len()).collect(),
                connected: true,
                entrant: Some(e),
            });
        }
    }
    let setup = BattleSetup {
        seed: random_seed(),
        map: None,
        units,
        statutes: STATUTES_PER_BATTLE,
        objective: Objective::Rout,
        loot: LootRules {
            chance: [LOOT_WAR, LOOT_WAR],
            boost: [2, 2],
            champion: None,
        },
    };
    let db = state.db.clone();
    let war_id = info.id;
    tokio::spawn(async move {
        if let Err(e) = wars::set_status(&db, war_id, "live").await {
            tracing::error!("stato guerra: {e}");
        }
    });
    tracing::info!(war = war_id, "guerra iniziata");
    launch(
        state,
        Mode::War,
        setup,
        players,
        roster_ids,
        None,
        Some(info),
    );
}

fn launch(
    state: AppState,
    mode: Mode,
    setup: BattleSetup,
    players: Vec<Player>,
    roster_ids: Vec<Option<i64>>,
    mission: Option<MissionInfo>,
    war: Option<WarInfo>,
) {
    let (tx, rx) = mpsc::channel(64);
    {
        let mut busy = state.busy.lock().unwrap();
        for (seat, p) in players.iter().enumerate() {
            if let Some(e) = p.human() {
                busy.insert(e.account_id);
                let _ = e.conn.send(ToConn::Joined {
                    input: tx.clone(),
                    seat,
                    team: p.team,
                });
            }
        }
    }
    drop(tx);
    tokio::spawn(run(
        state, mode, players, setup, roster_ids, mission, war, rx,
    ));
}

fn broadcast(players: &[Player], msg: &ServerMsg) {
    for e in players.iter().filter_map(Player::human) {
        e.send(msg.clone());
    }
}

fn update(players: &[Player], b: &Battle, events: Vec<Event>) {
    if !events.is_empty() {
        broadcast(
            players,
            &ServerMsg::Update {
                events,
                snapshot: b.snapshot(),
            },
        );
    }
}

/// L'IA completa il turno corrente.
fn ai_turn(b: &mut Battle, actions: &mut Vec<Action>) -> Vec<Event> {
    let turn = b.turn();
    let mut events = Vec::new();
    for a in ai::plan_turn(b) {
        if b.is_over() || b.turn() != turn {
            break;
        }
        events.extend(b.apply(a).expect("azione IA non valida"));
        actions.push(a);
    }
    events
}

fn owner_of(players: &[Player], unit: usize) -> usize {
    players
        .iter()
        .position(|p| p.units.contains(&unit))
        .unwrap_or(0)
}

#[allow(clippy::too_many_arguments)]
async fn run(
    state: AppState,
    mode: Mode,
    mut players: Vec<Player>,
    setup: BattleSetup,
    roster_ids: Vec<Option<i64>>,
    mission: Option<MissionInfo>,
    war: Option<WarInfo>,
    mut rx: mpsc::Receiver<MatchInput>,
) {
    let started = now();
    let (mut battle, first) = Battle::new(&setup);
    let mut actions: Vec<Action> = Vec::new();
    let turn_time = Duration::from_secs(state.config.turn_secs);
    let ai_delay = Duration::from_millis(state.config.ai_delay_ms);

    for p in &players {
        if let Some(e) = p.human() {
            let names = |same: bool| -> Vec<String> {
                players
                    .iter()
                    .filter(|o| (o.team == p.team) == same)
                    .filter_map(|o| o.human().map(|h| h.username.clone()))
                    .filter(|n| n != &e.username)
                    .collect()
            };
            let opponents = names(false);
            e.send(ServerMsg::Matched {
                mode,
                team: p.team,
                opponent: if opponents.is_empty() {
                    "IA".to_string()
                } else {
                    opponents.join(", ")
                },
                allies: names(true),
                mine: p.units.clone(),
                mission: mission.clone(),
                war: war.clone(),
                snapshot: battle.snapshot(),
            });
        }
    }
    update(&players, &battle, first);

    while !battle.is_over() {
        let active = battle.active_index().unwrap_or(0);
        let seat = owner_of(&players, active);
        if !players[seat].connected {
            if !ai_delay.is_zero() {
                sleep(ai_delay).await;
            }
            // Rese e disconnessioni arrivate nel frattempo vanno lette comunque.
            while let Ok(input) = rx.try_recv() {
                handle_input(mode, &mut battle, &mut actions, &mut players, None, input);
            }
            if battle.is_over() {
                break;
            }
            let ev = ai_turn(&mut battle, &mut actions);
            update(&players, &battle, ev);
            continue;
        }

        let turn = battle.turn();
        // Il gioco è a turni: chi gioca da solo ha tutto il tempo che vuole.
        // Il limite serve solo quando altri umani aspettano la mossa.
        let humans = players.iter().filter(|p| p.human().is_some()).count();
        let limit = if humans > 1 {
            turn_time
        } else {
            Duration::from_secs(7 * 86_400)
        };
        let deadline = Instant::now() + limit;
        loop {
            match timeout_at(deadline, rx.recv()).await {
                Err(_) => {
                    // Tempo scaduto: l'IA conclude il turno al posto del giocatore.
                    let ev = ai_turn(&mut battle, &mut actions);
                    update(&players, &battle, ev);
                    break;
                }
                Ok(None) => {
                    for p in &mut players {
                        p.connected = false;
                    }
                    break;
                }
                Ok(Some(input)) => {
                    handle_input(
                        mode,
                        &mut battle,
                        &mut actions,
                        &mut players,
                        Some(seat),
                        input,
                    );
                }
            }
            if battle.is_over() || battle.turn() != turn || !players[seat].connected {
                break;
            }
        }
    }

    let accounts: Vec<i64> = players
        .iter()
        .filter_map(Player::human)
        .map(|e| e.account_id)
        .collect();
    let saved = stats::save_match(
        &state.db,
        mode,
        &setup,
        &actions,
        &battle,
        &players,
        &roster_ids,
        started,
        state.config.season_days,
        mission.as_ref(),
        war.as_ref(),
    )
    .await;
    match saved {
        Ok(saved) => {
            for (i, p) in players.iter().enumerate() {
                if let (Some(e), Some(o)) = (p.human(), &saved.players[i]) {
                    e.send(ServerMsg::Ended {
                        match_id: saved.match_id,
                        winner: battle.winner().flatten(),
                        result: o.result.to_string(),
                        elo_before: o.elo_before,
                        elo_after: o.elo_after,
                        coins: o.coins,
                        season_before: o.season_before,
                        season_after: o.season_after,
                        clan_points: o.clan_points,
                        loot: o
                            .loot
                            .iter()
                            .map(|&i| {
                                serde_json::to_value(crate::armory::item_info(i))
                                    .unwrap_or_default()
                            })
                            .collect(),
                        rewards: o.rewards.clone(),
                    });
                }
            }
            tracing::info!(
                match_id = saved.match_id,
                mode = mode.key(),
                turns = battle.turn(),
                "partita conclusa"
            );
        }
        Err(e) => {
            tracing::error!("salvataggio partita fallito: {e}");
            broadcast(
                &players,
                &ServerMsg::Error {
                    message: "salvataggio della partita fallito".into(),
                },
            );
        }
    }
    let mut busy = state.busy.lock().unwrap();
    for a in accounts {
        busy.remove(&a);
    }
}

/// Gestisce un input. `on_turn`: il giocatore che controlla l'unità di turno
/// (se si sta aspettando un umano).
fn handle_input(
    mode: Mode,
    battle: &mut Battle,
    actions: &mut Vec<Action>,
    players: &mut [Player],
    on_turn: Option<usize>,
    input: MatchInput,
) {
    let seat = input.seat;
    let Some(team) = players.get(seat).map(|p| p.team) else {
        return;
    };
    let error = |players: &[Player], m: &str| {
        if let Some(e) = players[seat].human() {
            e.send(ServerMsg::Error {
                message: m.to_string(),
            });
        }
    };
    match input.kind {
        InputKind::Disconnected => players[seat].connected = false,
        InputKind::Leave => {
            players[seat].connected = false;
            let humans_left = players.iter().any(|p| p.team == team && p.connected);
            // In cooperativa e in guerra le unità passano all'IA; la squadra si
            // arrende solo quando non resta nessun umano. Altrove abbandonare è una resa.
            if !matches!(mode, Mode::Coop | Mode::War) || !humans_left {
                let a = Action::Concede(team);
                if let Ok(ev) = battle.apply(a) {
                    actions.push(a);
                    update(players, battle, ev);
                }
            }
        }
        InputKind::Action(Action::Concede(_)) => {
            // La resa passa sempre da `Leave`, così vale la regola della cooperativa.
            error(players, "per arrenderti abbandona la partita");
        }
        InputKind::Action(a) => {
            if on_turn != Some(seat) {
                error(players, "non è il turno di una tua unità");
                return;
            }
            match battle.apply(a) {
                Ok(ev) => {
                    actions.push(a);
                    update(players, battle, ev);
                }
                Err(e) => error(players, &e.to_string()),
            }
        }
    }
}

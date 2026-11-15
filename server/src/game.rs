//! Esecuzione di una partita: un task per partita possiede la `Battle`,
//! riceve le intenzioni dei giocatori, le valida, fa giocare l'IA quando
//! serve e alla fine salva replay, statistiche, esperienza ed ELO.

use std::time::Duration;

use tactica_core::ai;
use tactica_core::{Action, Battle, BattleSetup, Event};
use tokio::sync::mpsc;
use tokio::time::{Instant, sleep, timeout_at};

use crate::AppState;
use crate::db::now;
use crate::lobby::{Entrant, ToConn};
use crate::protocol::{Mode, ServerMsg};
use crate::stats;

pub struct MatchInput {
    pub team: u8,
    pub kind: InputKind,
}

pub enum InputKind {
    Action(Action),
    Disconnected,
}

pub enum Seat {
    Human(Entrant),
    Ai,
}

impl Seat {
    fn human(&self) -> Option<&Entrant> {
        match self {
            Seat::Human(e) => Some(e),
            Seat::Ai => None,
        }
    }
}

pub const STATUTES_PER_BATTLE: usize = 2;
const AI_NAMES: [&str; 4] = ["Sentinella", "Predone", "Veggente", "Eremita"];

fn random_seed() -> u64 {
    let mut b = [0u8; 8];
    getrandom::fill(&mut b).expect("sorgente casuale del sistema");
    u64::from_le_bytes(b)
}

/// Avvia una partita in un task dedicato.
pub fn start_match(state: AppState, mode: Mode, seats: [Seat; 2]) {
    // Squadre: umani dal roster; l'IA rispecchia classi, livelli ed equipaggiamento dell'avversario.
    let mut units = Vec::new();
    let mut roster_ids = Vec::new();
    for team in 0..2u8 {
        match &seats[team as usize] {
            Seat::Human(e) => {
                for r in &e.units {
                    units.push(r.to_setup(team));
                    roster_ids.push(Some(r.id));
                }
            }
            Seat::Ai => {
                let other = seats[1 - team as usize].human().expect("almeno un umano");
                for (i, r) in other.units.iter().enumerate() {
                    let mut u = r.to_setup(team);
                    u.name = AI_NAMES[i % AI_NAMES.len()].to_string();
                    u.xp = 0;
                    units.push(u);
                    roster_ids.push(None);
                }
            }
        }
    }
    let setup = BattleSetup {
        seed: random_seed(),
        map: None,
        units,
        statutes: STATUTES_PER_BATTLE,
    };

    let (tx, rx) = mpsc::channel(64);
    {
        let mut busy = state.busy.lock().unwrap();
        for (team, seat) in seats.iter().enumerate() {
            if let Some(e) = seat.human() {
                busy.insert(e.account_id);
                let _ = e.conn.send(ToConn::Joined {
                    input: tx.clone(),
                    team: team as u8,
                });
            }
        }
    }
    drop(tx);
    tokio::spawn(run(state, mode, seats, setup, roster_ids, rx));
}

fn broadcast(seats: &[Seat; 2], msg: &ServerMsg) {
    for e in seats.iter().filter_map(Seat::human) {
        e.send(msg.clone());
    }
}

fn update(seats: &[Seat; 2], b: &Battle, events: Vec<Event>) {
    if !events.is_empty() {
        broadcast(
            seats,
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

async fn run(
    state: AppState,
    mode: Mode,
    seats: [Seat; 2],
    setup: BattleSetup,
    roster_ids: Vec<Option<i64>>,
    mut rx: mpsc::Receiver<MatchInput>,
) {
    let started = now();
    let (mut battle, first) = Battle::new(&setup);
    let mut actions: Vec<Action> = Vec::new();
    let mut connected = [seats[0].human().is_some(), seats[1].human().is_some()];
    let turn_time = Duration::from_secs(state.config.turn_secs);
    let ai_delay = Duration::from_millis(state.config.ai_delay_ms);

    for (team, seat) in seats.iter().enumerate() {
        if let Some(e) = seat.human() {
            let opponent = seats[1 - team]
                .human()
                .map_or_else(|| "IA".to_string(), |o| o.username.clone());
            e.send(ServerMsg::Matched {
                mode,
                team: team as u8,
                opponent,
                snapshot: battle.snapshot(),
            });
        }
    }
    update(&seats, &battle, first);

    while !battle.is_over() {
        let team = battle.active_unit().map_or(0, |u| u.team) as usize;
        if !connected[team] {
            if !ai_delay.is_zero() {
                sleep(ai_delay).await;
            }
            // Le rese o disconnessioni arrivate nel frattempo vanno lette comunque.
            while let Ok(input) = rx.try_recv() {
                handle_out_of_turn(&mut battle, &mut actions, &mut connected, &seats, input);
            }
            if battle.is_over() {
                break;
            }
            let ev = ai_turn(&mut battle, &mut actions);
            update(&seats, &battle, ev);
            continue;
        }

        let turn = battle.turn();
        let deadline = Instant::now() + turn_time;
        loop {
            match timeout_at(deadline, rx.recv()).await {
                Err(_) => {
                    // Tempo scaduto: l'IA conclude il turno al posto del giocatore.
                    let ev = ai_turn(&mut battle, &mut actions);
                    update(&seats, &battle, ev);
                    break;
                }
                Ok(None) => {
                    connected = [false, false];
                    break;
                }
                Ok(Some(input)) if input.team as usize != team => {
                    handle_out_of_turn(&mut battle, &mut actions, &mut connected, &seats, input);
                }
                Ok(Some(MatchInput {
                    kind: InputKind::Disconnected,
                    ..
                })) => {
                    connected[team] = false;
                    break;
                }
                Ok(Some(MatchInput {
                    kind: InputKind::Action(a),
                    ..
                })) => match battle.apply(a) {
                    Ok(ev) => {
                        actions.push(a);
                        update(&seats, &battle, ev);
                    }
                    Err(e) => {
                        if let Some(p) = seats[team].human() {
                            p.send(ServerMsg::Error {
                                message: e.to_string(),
                            });
                        }
                    }
                },
            }
            if battle.is_over() || battle.turn() != turn {
                break;
            }
        }
    }

    let accounts: Vec<i64> = seats
        .iter()
        .filter_map(Seat::human)
        .map(|e| e.account_id)
        .collect();
    match stats::save_match(
        &state.db,
        mode,
        &setup,
        &actions,
        &battle,
        &seats,
        &roster_ids,
        started,
    )
    .await
    {
        Ok(saved) => {
            for (team, seat) in seats.iter().enumerate() {
                if let (Some(e), Some(o)) = (seat.human(), &saved.seats[team]) {
                    e.send(ServerMsg::Ended {
                        match_id: saved.match_id,
                        winner: battle.winner().flatten(),
                        result: o.result.to_string(),
                        elo_before: o.elo_before,
                        elo_after: o.elo_after,
                        coins: o.coins,
                        loot: o.loot.map(|i| i.key().to_string()),
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
                &seats,
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

/// Input di chi non è di turno: è accettata solo la resa (o la disconnessione).
fn handle_out_of_turn(
    battle: &mut Battle,
    actions: &mut Vec<Action>,
    connected: &mut [bool; 2],
    seats: &[Seat; 2],
    input: MatchInput,
) {
    let t = input.team as usize;
    match input.kind {
        InputKind::Disconnected => connected[t] = false,
        InputKind::Action(a @ Action::Concede(team)) if team == input.team => {
            if let Ok(ev) = battle.apply(a) {
                actions.push(a);
                update(seats, battle, ev);
            }
        }
        InputKind::Action(_) => {
            if let Some(p) = seats[t].human() {
                p.send(ServerMsg::Error {
                    message: "non è il tuo turno".into(),
                });
            }
        }
    }
}

//! Test end-to-end: server reale su porta casuale, client HTTP + WebSocket
//! che giocano usando l'IA del core su snapshot ricevuti dal server.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tactica_core::ai;
use tactica_core::{Action, Battle, BattleSetup};
use tactica_server::protocol::{ClientMsg, Mode, ServerMsg};
use tactica_server::{AppState, Config, router};
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

struct Server {
    base: String,
    ws: String,
    http: reqwest::Client,
}

async fn start() -> Server {
    let config = Config {
        addr: "127.0.0.1:0".into(),
        db_url: "sqlite::memory:".into(),
        turn_secs: 10,
        ai_delay_ms: 0,
        session_days: 1,
        backup_dir: None,
        backup_every_hours: 24,
        backup_keep: 1,
    };
    let state = AppState::new(config).await.unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router(state)).await.unwrap() });
    Server {
        base: format!("http://{addr}"),
        ws: format!("ws://{addr}/ws"),
        http: reqwest::Client::new(),
    }
}

impl Server {
    async fn post(&self, path: &str, token: Option<&str>, body: Value) -> (u16, Value) {
        let mut req = self.http.post(format!("{}{path}", self.base)).json(&body);
        if let Some(t) = token {
            req = req.bearer_auth(t);
        }
        let res = req.send().await.unwrap();
        let status = res.status().as_u16();
        (status, res.json().await.unwrap_or(Value::Null))
    }

    async fn get(&self, path: &str, token: Option<&str>) -> (u16, Value) {
        let mut req = self.http.get(format!("{}{path}", self.base));
        if let Some(t) = token {
            req = req.bearer_auth(t);
        }
        let res = req.send().await.unwrap();
        let status = res.status().as_u16();
        (status, res.json().await.unwrap_or(Value::Null))
    }

    /// Registra un account con un roster di 4 unità (una per classe).
    async fn player(&self, name: &str) -> (String, Vec<i64>) {
        let (st, s) = self
            .post(
                "/api/register",
                None,
                json!({"username": name, "password": "password-lunga"}),
            )
            .await;
        assert_eq!(st, 200, "{s}");
        let token = s["token"].as_str().unwrap().to_string();
        let mut ids = Vec::new();
        for (i, class) in ["vanguard", "scout", "arcanist", "mender"]
            .iter()
            .enumerate()
        {
            let (st, u) = self
                .post(
                    "/api/roster",
                    Some(&token),
                    json!({"name": format!("{name}{i}"), "class": class}),
                )
                .await;
            assert_eq!(st, 200, "{u}");
            ids.push(u["id"].as_i64().unwrap());
        }
        (token, ids)
    }
}

/// Gioca una partita: a ogni aggiornamento in cui tocca alla propria squadra
/// invia la prossima azione pianificata dall'IA sullo snapshot ricevuto.
/// `concede_after`: arrendersi dopo N azioni proprie.
async fn play(
    ws_url: String,
    token: String,
    mode: Mode,
    units: Vec<i64>,
    concede_after: Option<usize>,
) -> ServerMsg {
    let (mut ws, _) = connect_async(format!("{ws_url}?token={token}"))
        .await
        .unwrap();
    let send = |m: ClientMsg| Message::text(serde_json::to_string(&m).unwrap());
    ws.send(send(ClientMsg::Queue { mode, units }))
        .await
        .unwrap();
    let mut team = None;
    let mut sent = 0usize;
    loop {
        let msg = tokio::time::timeout(Duration::from_secs(30), ws.next())
            .await
            .expect("il server non risponde")
            .expect("connessione chiusa")
            .unwrap();
        let Message::Text(t) = msg else { continue };
        let m: ServerMsg = serde_json::from_str(t.as_str()).unwrap();
        match &m {
            ServerMsg::Matched { team: t, .. } => team = Some(*t),
            ServerMsg::Update { snapshot, .. } => {
                let mine = snapshot
                    .active
                    .is_some_and(|i| Some(snapshot.units[i].team) == team);
                if mine && !snapshot.finished {
                    let action = if concede_after.is_some_and(|n| sent >= n) {
                        Action::Concede(team.unwrap())
                    } else {
                        let view = Battle::from_snapshot(snapshot.clone());
                        ai::plan_turn(&view)[0]
                    };
                    ws.send(send(ClientMsg::Action { action })).await.unwrap();
                    sent += 1;
                }
            }
            ServerMsg::Error { message } => panic!("errore dal server: {message}"),
            ServerMsg::Ended { .. } => return m,
            ServerMsg::Welcome { .. } | ServerMsg::Queued { .. } => {}
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn full_flow() {
    let srv = start().await;

    // Account e accesso.
    let (alice, alice_units) = srv.player("alice").await;
    let (bob, bob_units) = srv.player("bob").await;
    let (st, _) = srv
        .post(
            "/api/register",
            None,
            json!({"username": "ALICE", "password": "password-lunga"}),
        )
        .await;
    assert_eq!(st, 409, "nome utente duplicato (case-insensitive)");
    let (st, _) = srv
        .post(
            "/api/login",
            None,
            json!({"username": "alice", "password": "sbagliata!"}),
        )
        .await;
    assert_eq!(st, 401);
    let (st, _) = srv.get("/api/me", None).await;
    assert_eq!(st, 401);
    let (st, me) = srv.get("/api/me", Some(&alice)).await;
    assert_eq!(st, 200);
    assert_eq!(me["roster"].as_array().unwrap().len(), 4);

    // Unità altrui rifiutate.
    let (mut ws, _) = connect_async(format!("{}?token={alice}", srv.ws))
        .await
        .unwrap();
    let q = ClientMsg::Queue {
        mode: Mode::Pve,
        units: vec![bob_units[0]],
    };
    ws.send(Message::text(serde_json::to_string(&q).unwrap()))
        .await
        .unwrap();
    let mut got_error = false;
    for _ in 0..3 {
        if let Some(Ok(Message::Text(t))) = ws.next().await
            && t.as_str().contains("\"error\"")
        {
            got_error = true;
            break;
        }
    }
    assert!(got_error);
    drop(ws);

    // PvE: alice contro l'IA.
    let ended = play(
        srv.ws.clone(),
        alice.clone(),
        Mode::Pve,
        alice_units.clone(),
        None,
    )
    .await;
    let ServerMsg::Ended {
        match_id,
        rewards,
        elo_before,
        elo_after,
        ..
    } = ended
    else {
        unreachable!()
    };
    assert_eq!(rewards.len(), 4);
    assert!(rewards.iter().all(|r| r.xp > 0));
    assert_eq!(elo_before, elo_after, "il PvE non cambia l'ELO");

    // Il replay salvato riproduce la partita.
    let (st, m) = srv.get(&format!("/api/matches/{match_id}"), None).await;
    assert_eq!(st, 200);
    let setup: BattleSetup = serde_json::from_value(m["setup"].clone()).unwrap();
    let actions: Vec<Action> = serde_json::from_value(m["actions"].clone()).unwrap();
    let (mut replay, _) = Battle::new(&setup);
    for a in actions {
        replay.apply(a).unwrap();
    }
    assert_eq!(
        format!("{:016x}", replay.state_hash()),
        m["final_hash"].as_str().unwrap()
    );
    assert_eq!(
        replay.winner().flatten().map(i64::from),
        m["winner_team"].as_i64()
    );

    // Il roster è progredito.
    let (_, me) = srv.get("/api/me", Some(&alice)).await;
    let xp_or_level: i64 = me["roster"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| u["xp"].as_i64().unwrap() + 100 * (u["level"].as_i64().unwrap() - 1))
        .sum();
    assert!(xp_or_level > 0);

    // PvP: alice contro bob, entrambi guidati dall'IA del client.
    let a = tokio::spawn(play(
        srv.ws.clone(),
        alice.clone(),
        Mode::Pvp,
        alice_units.clone(),
        None,
    ));
    tokio::time::sleep(Duration::from_millis(200)).await;
    let b = tokio::spawn(play(
        srv.ws.clone(),
        bob.clone(),
        Mode::Pvp,
        bob_units.clone(),
        None,
    ));
    let (ea, eb) = (a.await.unwrap(), b.await.unwrap());
    let (
        ServerMsg::Ended {
            match_id: ma,
            elo_before: ab,
            elo_after: aa,
            result: ra,
            ..
        },
        ServerMsg::Ended {
            match_id: mb,
            elo_before: bb,
            elo_after: ba,
            result: rb,
            ..
        },
    ) = (ea, eb)
    else {
        unreachable!()
    };
    assert_eq!(ma, mb);
    assert_eq!((aa - ab) + (ba - bb), 0, "ELO a somma zero");
    if ra != "draw" {
        assert_ne!(ra, rb);
    }

    // PvP con resa: bob si arrende dopo 3 azioni.
    let a = tokio::spawn(play(
        srv.ws.clone(),
        alice.clone(),
        Mode::Pvp,
        alice_units.clone(),
        None,
    ));
    tokio::time::sleep(Duration::from_millis(200)).await;
    let b = tokio::spawn(play(
        srv.ws.clone(),
        bob.clone(),
        Mode::Pvp,
        bob_units.clone(),
        Some(3),
    ));
    let (ea, eb) = (a.await.unwrap(), b.await.unwrap());
    let (ServerMsg::Ended { result: ra, .. }, ServerMsg::Ended { result: rb, .. }) = (ea, eb)
    else {
        unreachable!()
    };
    assert_eq!((ra.as_str(), rb.as_str()), ("win", "loss"));

    // Statistiche personali, classifica e globali.
    let (st, p) = srv.get("/api/players/alice", None).await;
    assert_eq!(st, 200);
    assert_eq!(
        p["wins"].as_i64().unwrap() + p["losses"].as_i64().unwrap() + p["draws"].as_i64().unwrap(),
        3
    );
    assert_eq!(p["totals"]["units_fielded"].as_i64().unwrap(), 12);
    assert_eq!(p["recent"].as_array().unwrap().len(), 3);

    let (_, lb) = srv.get("/api/leaderboard", None).await;
    assert_eq!(lb.as_array().unwrap().len(), 2);

    let (_, g) = srv.get("/api/stats", None).await;
    assert_eq!(g["matches"].as_i64().unwrap(), 3);
    assert_eq!(g["pvp_matches"].as_i64().unwrap(), 2);
    assert_eq!(g["classes"].as_array().unwrap().len(), 4);
    assert!(!g["statutes"].as_array().unwrap().is_empty());
}

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
    db: sqlx::SqlitePool,
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
        season_days: 28,
    };
    let state = AppState::new(config).await.unwrap();
    let db = state.db.clone();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router(state)).await.unwrap() });
    Server {
        db,
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

    async fn put(&self, path: &str, token: &str, body: Value) -> (u16, Value) {
        let res = self
            .http
            .put(format!("{}{path}", self.base))
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .unwrap();
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

    // Progressione: kit iniziale, negozio, equipaggiamento, classi.
    let (_, me) = srv.get("/api/me", Some(&alice)).await;
    assert_eq!(me["coins"].as_i64(), Some(400));
    let arcanist = &me["roster"][2];
    assert_eq!(arcanist["class"], "arcanist");
    assert_eq!(arcanist["equipment"], json!(["rod", "leather"]));
    assert_eq!(arcanist["usable"], json!(["strike", "flame"]));
    let arc_id = alice_units[2];
    let vanguard_id = alice_units[0];

    let (st, _) = srv
        .post(
            "/api/shop/buy",
            Some(&alice),
            json!({"item": "cinder_staff"}),
        )
        .await;
    assert_eq!(st, 400, "oggetto solo da bottino");
    let (st, inv) = srv
        .post("/api/shop/buy", Some(&alice), json!({"item": "frost_rod"}))
        .await;
    assert_eq!(st, 200, "{inv}");
    assert_eq!(inv["coins"].as_i64(), Some(50));
    let (st, _) = srv
        .post("/api/shop/buy", Some(&alice), json!({"item": "storm_rod"}))
        .await;
    assert_eq!(st, 409, "monete insufficienti");

    let path = |id: i64, what: &str| format!("/api/roster/{id}/{what}");
    let (st, _) = srv
        .put(
            &path(vanguard_id, "equipment"),
            &alice,
            json!({"items": ["frost_rod"]}),
        )
        .await;
    assert_eq!(st, 400, "oggetto non adatto alla classe");
    let (st, _) = srv
        .put(
            &path(arc_id, "equipment"),
            &alice,
            json!({"items": ["storm_rod"]}),
        )
        .await;
    assert_eq!(st, 409, "oggetto non posseduto");
    let (st, u) = srv
        .put(
            &path(arc_id, "equipment"),
            &alice,
            json!({"items": ["frost_rod", "leather"]}),
        )
        .await;
    assert_eq!(st, 200, "{u}");
    assert_eq!(u["usable"], json!(["strike", "flame", "frost"]));
    assert_eq!(u["abilities"][0]["key"], "frost");
    assert_eq!(u["abilities"][0]["training"], true);
    // La verga iniziale è tornata nell'inventario: si può vendere.
    let (st, inv) = srv
        .post("/api/shop/sell", Some(&alice), json!({"item": "rod"}))
        .await;
    assert_eq!(st, 200, "{inv}");
    assert_eq!(inv["coins"].as_i64(), Some(100));

    let (st, _) = srv
        .put(
            &path(vanguard_id, "class"),
            &alice,
            json!({"class": "runeblade"}),
        )
        .await;
    assert_eq!(st, 403, "classe avanzata non ancora sbloccata");
    let (st, _) = srv
        .post(
            "/api/roster",
            Some(&alice),
            json!({"name": "X", "class": "warden"}),
        )
        .await;
    assert_eq!(st, 400, "niente reclute in classe avanzata");
    let (st, u) = srv
        .put(
            &path(vanguard_id, "class"),
            &alice,
            json!({"class": "scout"}),
        )
        .await;
    assert_eq!(st, 200, "{u}");
    assert_eq!(
        u["equipment"],
        json!(["leather"]),
        "la spada torna nell'inventario"
    );
    let (st, _) = srv
        .put(
            &path(vanguard_id, "class"),
            &alice,
            json!({"class": "vanguard"}),
        )
        .await;
    assert_eq!(st, 200);
    let (st, _) = srv
        .put(
            &path(vanguard_id, "equipment"),
            &alice,
            json!({"items": ["iron_sword", "leather"]}),
        )
        .await;
    assert_eq!(st, 200);

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
        coins,
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

    // Risonanza: il Gelo dell'Arcanista ha accumulato RP; monete accreditate.
    let arc_reward = rewards.iter().find(|r| r.roster_id == arc_id).unwrap();
    assert_eq!(arc_reward.rp.len(), 1);
    assert_eq!(arc_reward.rp[0].ability, "frost");
    assert!(arc_reward.rp[0].gained >= 20);
    assert!(coins >= 40);
    let (_, me) = srv.get("/api/me", Some(&alice)).await;
    assert_eq!(me["coins"].as_i64(), Some(100 + coins));
    let frost = &me["roster"][2]["abilities"][0];
    assert_eq!(frost["rp"].as_i64(), Some(arc_reward.rp[0].rp));

    // Il roster è progredito.
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn clans_and_seasons() {
    use tactica_server::seasons;

    let srv = start().await;
    let (alice, alice_units) = srv.player("alice").await;
    let (bob, _) = srv.player("bob").await;
    let (carol, _) = srv.player("carol").await;
    let (dave, dave_units) = srv.player("dave").await;

    // Fondazione (costa monete) e regole sui nomi.
    let (st, _) = srv
        .post(
            "/api/clans",
            Some(&alice),
            json!({"name": "X", "tag": "LAME"}),
        )
        .await;
    assert_eq!(st, 400);
    let (st, c) = srv
        .post(
            "/api/clans",
            Some(&alice),
            json!({"name": "Lame d'Argento", "tag": "lame", "motto": "Insieme"}),
        )
        .await;
    assert_eq!(st, 200, "{c}");
    assert_eq!(c["tag"], "LAME");
    let (_, me) = srv.get("/api/me", Some(&alice)).await;
    assert_eq!(me["coins"].as_i64(), Some(200));
    assert_eq!(me["clan"]["role"], "leader");
    let (st, _) = srv
        .post(
            "/api/clans",
            Some(&bob),
            json!({"name": "Lame d'argento", "tag": "LA2"}),
        )
        .await;
    assert_eq!(st, 409, "nome già in uso (maiuscole indifferenti)");

    // Adesione libera, poi clan chiuso: serve un invito.
    let (st, _) = srv
        .post("/api/clans/LAME/join", Some(&bob), json!({}))
        .await;
    assert_eq!(st, 200);
    let (st, _) = srv
        .put("/api/clans/LAME", &alice, json!({"open": false}))
        .await;
    assert_eq!(st, 200);
    let (st, _) = srv
        .post("/api/clans/LAME/join", Some(&carol), json!({}))
        .await;
    assert_eq!(st, 403);
    let (st, _) = srv
        .post(
            "/api/clans/LAME/invite",
            Some(&bob),
            json!({"username": "carol"}),
        )
        .await;
    assert_eq!(st, 403, "un semplice membro non invita");
    let (st, _) = srv
        .post(
            "/api/clans/LAME/invite",
            Some(&alice),
            json!({"username": "carol"}),
        )
        .await;
    assert_eq!(st, 200);
    let (_, me) = srv.get("/api/me", Some(&carol)).await;
    assert_eq!(me["invites"], json!(["LAME"]));
    let (st, _) = srv
        .post("/api/clans/LAME/join", Some(&carol), json!({}))
        .await;
    assert_eq!(st, 200);

    // Ruoli ed espulsioni.
    let (st, _) = srv
        .post(
            "/api/clans/LAME/role",
            Some(&alice),
            json!({"username": "bob", "role": "officer"}),
        )
        .await;
    assert_eq!(st, 200);
    let (st, _) = srv
        .post(
            "/api/clans/LAME/kick",
            Some(&carol),
            json!({"username": "bob"}),
        )
        .await;
    assert_eq!(st, 403);
    let (st, _) = srv
        .post(
            "/api/clans/LAME/kick",
            Some(&bob),
            json!({"username": "alice"}),
        )
        .await;
    assert_eq!(st, 403, "un ufficiale non espelle il capo");
    let (st, _) = srv
        .post(
            "/api/clans/LAME/kick",
            Some(&bob),
            json!({"username": "carol"}),
        )
        .await;
    assert_eq!(st, 200);
    let (_, list) = srv.get("/api/clans", None).await;
    assert_eq!(list[0]["members"].as_i64(), Some(2));

    // PvP classificato: punteggio stagionale, punti clan e bonus monete.
    let a = tokio::spawn(play(
        srv.ws.clone(),
        alice.clone(),
        Mode::Pvp,
        alice_units.clone(),
        None,
    ));
    tokio::time::sleep(Duration::from_millis(200)).await;
    let d = tokio::spawn(play(
        srv.ws.clone(),
        dave.clone(),
        Mode::Pvp,
        dave_units.clone(),
        Some(2),
    ));
    let (ea, ed) = (a.await.unwrap(), d.await.unwrap());
    let ServerMsg::Ended {
        result,
        coins,
        season_before,
        season_after,
        clan_points,
        ..
    } = ea
    else {
        unreachable!()
    };
    assert_eq!(result, "win", "dave si arrende");
    assert_eq!(season_before, Some(1000));
    assert!(season_after.unwrap() > 1000);
    assert_eq!(clan_points, 3);
    assert_eq!(coins, 132, "120 + 10% di bonus clan");
    let ServerMsg::Ended {
        coins: dave_coins,
        clan_points: dave_clan,
        ..
    } = ed
    else {
        unreachable!()
    };
    assert_eq!((dave_coins, dave_clan), (40, 0));

    let (_, cur) = srv.get("/api/seasons/current", None).await;
    assert_eq!(cur["number"].as_i64(), Some(1));
    assert_eq!(cur["players"][0]["username"], "alice");
    assert_eq!(cur["players"][0]["clan"], "LAME");
    assert_eq!(cur["clans"][0]["tag"], "LAME");
    assert_eq!(cur["clans"][0]["points"].as_i64(), Some(3));
    let alice_rating = cur["players"][0]["rating"].as_i64().unwrap();

    // Chiusura della stagione: classifiche finali, titoli, premi, nuova stagione.
    let (_, before) = srv.get("/api/me", Some(&alice)).await;
    let closed = seasons::rollover_if_due(&srv.db, 28, tactica_server::db::now() + 30 * 86_400)
        .await
        .unwrap()
        .expect("stagione chiusa");
    assert_eq!(closed.number, 1);
    let (_, list) = srv.get("/api/seasons", None).await;
    assert_eq!(list.as_array().unwrap().len(), 2);
    assert_eq!(list[0]["number"].as_i64(), Some(2));
    let (_, s1) = srv.get("/api/seasons/1", None).await;
    assert_eq!(s1["closed"], true);
    let (_, p) = srv.get("/api/players/alice", None).await;
    let titles: Vec<&str> = p["titles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t.as_str().unwrap())
        .collect();
    assert!(titles.contains(&"Campione della Stagione 1"), "{titles:?}");
    assert!(
        titles.contains(&"Clan campione della Stagione 1"),
        "{titles:?}"
    );
    let (_, after) = srv.get("/api/me", Some(&alice)).await;
    assert_eq!(
        after["coins"].as_i64().unwrap() - before["coins"].as_i64().unwrap(),
        1000 + 300,
        "premio da primo classificato + clan campione"
    );
    assert_eq!(after["season"]["number"].as_i64(), Some(2));
    // Reset morbido nella nuova stagione.
    let mut conn = srv.db.acquire().await.unwrap();
    let s2 = seasons::current(&mut conn, 28).await.unwrap();
    let r2 = seasons::rating(&mut conn, &s2, after["id"].as_i64().unwrap())
        .await
        .unwrap();
    assert_eq!(r2, 1000 + (alice_rating - 1000) / 2);
    drop(conn);

    // Il capo esce: il comando passa all'ufficiale.
    let (st, _) = srv.post("/api/clans/leave", Some(&alice), json!({})).await;
    assert_eq!(st, 200);
    let (_, c) = srv.get("/api/clans/LAME", None).await;
    assert_eq!(c["members"][0]["username"], "bob");
    assert_eq!(c["members"][0]["role"], "leader");
    assert_eq!(c["history"][0]["rank"].as_i64(), Some(1));
    let (st, _) = srv.post("/api/clans/leave", Some(&bob), json!({})).await;
    assert_eq!(st, 200);
    let (st, _) = srv.get("/api/clans/LAME", None).await;
    assert_eq!(st, 404, "clan sciolto quando esce l'ultimo membro");
}

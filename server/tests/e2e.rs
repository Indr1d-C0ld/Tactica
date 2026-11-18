//! Test end-to-end: server reale su porta casuale, client HTTP + WebSocket
//! che giocano usando l'IA del core su snapshot ricevuti dal server.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
#[allow(unused_imports)]
use tactica_core::Objective;
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
    start_with(|_| {}).await
}

async fn start_with(tweak: impl FnOnce(&mut Config)) -> Server {
    let mut config = Config {
        addr: "127.0.0.1:0".into(),
        db_url: "sqlite::memory:".into(),
        turn_secs: 10,
        ai_delay_ms: 0,
        session_days: 1,
        backup_dir: None,
        backup_every_hours: 24,
        backup_keep: 1,
        season_days: 28,
        coop_min_players: 2,
        war_min_players: 1,
        auth_per_minute: 1000,
        register_per_hour: 1000,
        admins: vec!["boss".into()],
        tournament_match_hours: 72,
    };
    tweak(&mut config);
    let state = AppState::new(config).await.unwrap();
    let db = state.db.clone();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            router(state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap()
    });
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

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

fn text(m: ClientMsg) -> Message {
    Message::text(serde_json::to_string(&m).unwrap())
}

async fn connect(ws_url: &str, token: &str) -> Ws {
    connect_async(format!("{ws_url}?token={token}"))
        .await
        .unwrap()
        .0
}

async fn next_msg(ws: &mut Ws) -> ServerMsg {
    loop {
        let msg = tokio::time::timeout(Duration::from_secs(30), ws.next())
            .await
            .expect("il server non risponde")
            .expect("connessione chiusa")
            .unwrap();
        if let Message::Text(t) = msg {
            return serde_json::from_str(t.as_str()).unwrap();
        }
    }
}

/// Gioca una partita: a ogni aggiornamento in cui tocca a una propria unità
/// invia la prossima azione pianificata dall'IA sullo snapshot ricevuto.
/// `leave_after`: abbandona dopo N azioni proprie.
async fn game_loop(ws: &mut Ws, leave_after: Option<usize>) -> ServerMsg {
    let mut mine: Vec<usize> = Vec::new();
    let mut sent = 0usize;
    let mut left = false;
    loop {
        let m = next_msg(ws).await;
        match &m {
            ServerMsg::Matched { mine: units, .. } => mine = units.clone(),
            ServerMsg::Update { snapshot, .. } => {
                let my_turn = snapshot.active.is_some_and(|i| mine.contains(&i));
                if my_turn && !snapshot.finished && !left {
                    if leave_after.is_some_and(|n| sent >= n) {
                        ws.send(text(ClientMsg::Leave)).await.unwrap();
                        left = true;
                        continue;
                    }
                    let view = Battle::from_snapshot(snapshot.clone());
                    let action = ai::plan_turn(&view)[0];
                    ws.send(text(ClientMsg::Action { action })).await.unwrap();
                    sent += 1;
                }
            }
            ServerMsg::Error { message } => panic!("errore dal server: {message}"),
            ServerMsg::Ended { .. } => return m,
            _ => {}
        }
    }
}

async fn play(
    ws_url: String,
    token: String,
    mode: Mode,
    units: Vec<i64>,
    leave_after: Option<usize>,
) -> ServerMsg {
    let mut ws = connect(&ws_url, &token).await;
    ws.send(text(ClientMsg::Queue { mode, units }))
        .await
        .unwrap();
    game_loop(&mut ws, leave_after).await
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn coop_clan_mission() {
    let srv = start().await;
    let (alice, alice_units) = srv.player("alice").await;
    let (bob, bob_units) = srv.player("bob").await;
    let (carol, carol_units) = srv.player("carol").await;
    let (st, _) = srv
        .post(
            "/api/clans",
            Some(&alice),
            json!({"name": "Compagnia", "tag": "COMP"}),
        )
        .await;
    assert_eq!(st, 200);
    let (st, _) = srv
        .post("/api/clans/COMP/join", Some(&bob), json!({}))
        .await;
    assert_eq!(st, 200);

    // Senza clan niente stanze.
    let mut wc = connect(&srv.ws, &carol).await;
    wc.send(text(ClientMsg::RoomCreate {
        difficulty: 1,
        units: carol_units.clone(),
    }))
    .await
    .unwrap();
    loop {
        match next_msg(&mut wc).await {
            ServerMsg::Error { message } => {
                assert!(message.contains("clan"), "{message}");
                break;
            }
            ServerMsg::Welcome { .. } => {}
            other => panic!("inatteso: {other:?}"),
        }
    }

    // Alice apre una stanza; Bob la trova e ci entra.
    let mut wa = connect(&srv.ws, &alice).await;
    let mut wb = connect(&srv.ws, &bob).await;
    wa.send(text(ClientMsg::RoomCreate {
        difficulty: 2,
        units: alice_units.clone(),
    }))
    .await
    .unwrap();
    let room_id = loop {
        if let ServerMsg::Room { room: Some(r) } = next_msg(&mut wa).await {
            assert_eq!(r.clan, "COMP");
            assert_eq!(r.members.len(), 1);
            break r.id;
        }
    };
    // Avvio con un solo giocatore: rifiutato.
    wa.send(text(ClientMsg::RoomStart)).await.unwrap();
    loop {
        match next_msg(&mut wa).await {
            ServerMsg::Error { message } => {
                assert!(message.contains("giocatori"), "{message}");
                break;
            }
            ServerMsg::Room { .. } => {}
            other => panic!("inatteso: {other:?}"),
        }
    }
    wb.send(text(ClientMsg::RoomList)).await.unwrap();
    loop {
        if let ServerMsg::Rooms { rooms } = next_msg(&mut wb).await {
            assert_eq!(rooms.len(), 1);
            assert_eq!(rooms[0].id, room_id);
            break;
        }
    }
    wb.send(text(ClientMsg::RoomJoin {
        room: room_id,
        units: bob_units.clone(),
    }))
    .await
    .unwrap();
    loop {
        if let ServerMsg::Room { room: Some(r) } = next_msg(&mut wa).await
            && r.members.len() == 2
        {
            // 6 posti divisi fra 2 giocatori: 3 unità ciascuno.
            assert!(r.members.iter().all(|m| m.units == 3));
            break;
        }
    }
    // Solo il capostanza può avviare.
    wb.send(text(ClientMsg::RoomStart)).await.unwrap();
    loop {
        match next_msg(&mut wb).await {
            ServerMsg::Error { message } => {
                assert!(message.contains("capostanza"), "{message}");
                break;
            }
            ServerMsg::Room { .. } => {}
            other => panic!("inatteso: {other:?}"),
        }
    }
    wa.send(text(ClientMsg::RoomStart)).await.unwrap();

    // Entrambi giocano le proprie unità; Bob abbandona a metà e l'IA lo sostituisce.
    let a = tokio::spawn(async move {
        let (mode, mine, allies, mission, snapshot) = loop {
            if let ServerMsg::Matched {
                mode,
                mine,
                allies,
                mission,
                snapshot,
                ..
            } = next_msg(&mut wa).await
            {
                break (mode, mine, allies, mission, snapshot);
            }
        };
        assert_eq!(mode, Mode::Coop);
        assert_eq!(mine, vec![0, 1, 2]);
        assert_eq!(allies, vec!["bob".to_string()]);
        let mission = mission.expect("informazioni sulla missione");
        assert_eq!(mission.difficulty, 2);
        let enemies = snapshot.units.iter().filter(|u| u.team == 1).count();
        assert!(
            enemies >= 4,
            "difficoltà 2: almeno 4 nemici, trovati {enemies}"
        );
        if mission.hunt {
            assert!(matches!(snapshot.objective, Objective::Champion(_)));
        }
        game_loop_with(&mut wa, mine, None).await
    });
    let b = tokio::spawn(async move { game_loop(&mut wb, Some(4)).await });
    let (ea, eb) = (a.await.unwrap(), b.await.unwrap());
    let ServerMsg::Ended {
        match_id,
        result: ra,
        rewards: rwa,
        coins,
        clan_points,
        ..
    } = ea
    else {
        unreachable!()
    };
    let ServerMsg::Ended {
        match_id: mb,
        result: rb,
        rewards: rwb,
        ..
    } = eb
    else {
        unreachable!()
    };
    assert_eq!(match_id, mb);
    assert_eq!(ra, rb, "stessa squadra, stesso esito");
    assert_eq!(rwa.len(), 3, "ricompense solo per le proprie unità");
    assert_eq!(rwb.len(), 3);
    assert!(rwa.iter().all(|r| alice_units.contains(&r.roster_id)));
    assert!(rwb.iter().all(|r| bob_units.contains(&r.roster_id)));
    if ra == "win" {
        assert_eq!(
            coins,
            (150 + 50) * 110 / 100,
            "missione ardua vinta + bonus clan"
        );
        assert!(clan_points >= 2);
        let (_, c) = srv.get("/api/clans/COMP", None).await;
        assert_eq!(
            c["season_points"].as_i64(),
            Some(clan_points),
            "punti assegnati una sola volta"
        );
    }

    // Partita registrata con entrambi i giocatori e il replay riproducibile.
    let (_, m) = srv.get(&format!("/api/matches/{match_id}"), None).await;
    assert_eq!(m["mode"], "coop");
    assert_eq!(
        m["rules_version"].as_i64(),
        Some(tactica_core::RULES_VERSION as i64)
    );
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
    let (_, pa) = srv.get("/api/players/alice", None).await;
    let (_, pb) = srv.get("/api/players/bob", None).await;
    assert_eq!(pa["recent"][0]["mode"], "coop");
    assert_eq!(pb["recent"][0]["mode"], "coop");
}

/// Come `game_loop`, ma con le unità già note (il `Matched` è stato letto prima).
async fn game_loop_with(ws: &mut Ws, mine: Vec<usize>, leave_after: Option<usize>) -> ServerMsg {
    let mut sent = 0usize;
    loop {
        let m = next_msg(ws).await;
        match &m {
            ServerMsg::Update { snapshot, .. } => {
                if snapshot.active.is_some_and(|i| mine.contains(&i)) && !snapshot.finished {
                    if leave_after.is_some_and(|n| sent >= n) {
                        ws.send(text(ClientMsg::Leave)).await.unwrap();
                        continue;
                    }
                    let view = Battle::from_snapshot(snapshot.clone());
                    ws.send(text(ClientMsg::Action {
                        action: ai::plan_turn(&view)[0],
                    }))
                    .await
                    .unwrap();
                    sent += 1;
                }
            }
            ServerMsg::Error { message } => panic!("errore dal server: {message}"),
            ServerMsg::Ended { .. } => return m,
            _ => {}
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn clan_war() {
    let srv = start().await;
    let (alice, alice_units) = srv.player("alice").await;
    let (bob, bob_units) = srv.player("bob").await;
    let (carol, _) = srv.player("carol").await;
    assert_eq!(
        srv.post(
            "/api/clans",
            Some(&alice),
            json!({"name": "Lupi", "tag": "LUPI"})
        )
        .await
        .0,
        200
    );
    assert_eq!(
        srv.post(
            "/api/clans",
            Some(&bob),
            json!({"name": "Corvi", "tag": "CORV"})
        )
        .await
        .0,
        200
    );
    assert_eq!(
        srv.post("/api/clans/CORV/join", Some(&carol), json!({}))
            .await
            .0,
        200
    );

    // Dichiarazione: solo capi o ufficiali, non al proprio clan, una sola guerra aperta.
    let (st, _) = srv
        .post("/api/wars", Some(&carol), json!({"tag": "LUPI"}))
        .await;
    assert_eq!(st, 403);
    let (st, _) = srv
        .post("/api/wars", Some(&alice), json!({"tag": "LUPI"}))
        .await;
    assert_eq!(st, 400);
    let (st, w) = srv
        .post("/api/wars", Some(&alice), json!({"tag": "CORV"}))
        .await;
    assert_eq!(st, 200, "{w}");
    let war = w["id"].as_i64().unwrap();
    assert_eq!(w["status"], "pending");
    let (st, _) = srv
        .post("/api/wars", Some(&bob), json!({"tag": "LUPI"}))
        .await;
    assert_eq!(st, 409, "già una guerra aperta fra i due clan");

    // Lo sfidante non può accettare per l'altro; un membro semplice nemmeno.
    assert_eq!(
        srv.post(&format!("/api/wars/{war}/accept"), Some(&alice), json!({}))
            .await
            .0,
        403
    );
    assert_eq!(
        srv.post(&format!("/api/wars/{war}/accept"), Some(&carol), json!({}))
            .await
            .0,
        403
    );
    let (st, w) = srv
        .post(&format!("/api/wars/{war}/accept"), Some(&bob), json!({}))
        .await;
    assert_eq!(st, 200);
    assert_eq!(w["status"], "accepted");
    let (_, list) = srv.get("/api/wars", Some(&bob)).await;
    assert_eq!(list[0]["id"].as_i64(), Some(war));

    // Schieramenti: chi non è nei clan coinvolti non entra; via ai pronti.
    let (dave, dave_units) = srv.player("dave").await;
    let mut wd = connect(&srv.ws, &dave).await;
    wd.send(text(ClientMsg::WarJoin {
        war,
        units: dave_units,
    }))
    .await
    .unwrap();
    loop {
        match next_msg(&mut wd).await {
            ServerMsg::Error { message } => {
                assert!(message.contains("clan"), "{message}");
                break;
            }
            ServerMsg::Welcome { .. } => {}
            other => panic!("inatteso: {other:?}"),
        }
    }
    let mut wa = connect(&srv.ws, &alice).await;
    let mut wb = connect(&srv.ws, &bob).await;
    wa.send(text(ClientMsg::WarJoin {
        war,
        units: alice_units.clone(),
    }))
    .await
    .unwrap();
    wb.send(text(ClientMsg::WarJoin {
        war,
        units: bob_units.clone(),
    }))
    .await
    .unwrap();
    loop {
        if let ServerMsg::WarRoom { room: Some(r) } = next_msg(&mut wa).await
            && r.sides[0].members.len() == 1
            && r.sides[1].members.len() == 1
        {
            assert_eq!(
                (r.sides[0].tag.as_str(), r.sides[1].tag.as_str()),
                ("LUPI", "CORV")
            );
            // Un solo giocatore per lato: comanda tutti e 6 i posti (fino a 4 unità scelte).
            assert_eq!(r.sides[0].members[0].units, 4);
            break;
        }
    }
    wa.send(text(ClientMsg::WarReady { ready: true }))
        .await
        .unwrap();
    wb.send(text(ClientMsg::WarReady { ready: true }))
        .await
        .unwrap();

    let a = tokio::spawn(async move { game_loop(&mut wa, None).await });
    let b = tokio::spawn(async move { game_loop(&mut wb, Some(3)).await });
    let (ea, eb) = (a.await.unwrap(), b.await.unwrap());
    let ServerMsg::Ended {
        match_id,
        result: ra,
        coins: ca,
        clan_points: pa,
        elo_before,
        elo_after,
        ..
    } = ea
    else {
        unreachable!()
    };
    let ServerMsg::Ended { result: rb, .. } = eb else {
        unreachable!()
    };
    assert_eq!(
        (ra.as_str(), rb.as_str()),
        ("win", "loss"),
        "Bob abbandona: è l'unico umano dei Corvi"
    );
    assert_eq!(ca, 220, "200 monete di guerra + 10% del clan");
    assert_eq!(pa, 5);
    assert_eq!(elo_before, elo_after, "la guerra non tocca l'ELO personale");

    // Esito registrato: ELO dei clan a somma zero, record, punti stagionali.
    let (_, w) = srv.get("/api/wars", Some(&alice)).await;
    assert_eq!(w[0]["status"], "done");
    assert_eq!(w[0]["winner_tag"], "LUPI");
    assert_eq!(w[0]["match_id"].as_i64(), Some(match_id));
    let (_, lupi) = srv.get("/api/clans/LUPI", None).await;
    let (_, corvi) = srv.get("/api/clans/CORV", None).await;
    assert_eq!(
        lupi["rating"].as_i64().unwrap() + corvi["rating"].as_i64().unwrap(),
        2000
    );
    assert!(lupi["rating"].as_i64().unwrap() > 1000);
    assert_eq!(
        (lupi["war_wins"].as_i64(), corvi["war_losses"].as_i64()),
        (Some(1), Some(1))
    );
    assert_eq!(lupi["season_points"].as_i64(), Some(5));
    let (_, m) = srv.get(&format!("/api/matches/{match_id}"), None).await;
    assert_eq!(m["mode"], "war");

    // Conclusa la guerra se ne può dichiarare un'altra; si può anche annullare.
    let (st, w2) = srv
        .post("/api/wars", Some(&bob), json!({"tag": "LUPI"}))
        .await;
    assert_eq!(st, 200);
    let id2 = w2["id"].as_i64().unwrap();
    let (st, w2) = srv
        .post(&format!("/api/wars/{id2}/cancel"), Some(&alice), json!({}))
        .await;
    assert_eq!(st, 200);
    assert_eq!(w2["status"], "cancelled");
}

/// Combatte una guerra già accettata: il secondo giocatore abbandona dopo 2 azioni.
async fn fight(
    srv: &Server,
    war: i64,
    a: (&str, &[i64]),
    b: (&str, &[i64]),
) -> (ServerMsg, ServerMsg) {
    let mut wa = connect(&srv.ws, a.0).await;
    let mut wb = connect(&srv.ws, b.0).await;
    wa.send(text(ClientMsg::WarJoin {
        war,
        units: a.1.to_vec(),
    }))
    .await
    .unwrap();
    wb.send(text(ClientMsg::WarJoin {
        war,
        units: b.1.to_vec(),
    }))
    .await
    .unwrap();
    loop {
        if let ServerMsg::WarRoom { room: Some(r) } = next_msg(&mut wa).await
            && r.sides.iter().all(|s| s.members.len() == 1)
        {
            break;
        }
    }
    wa.send(text(ClientMsg::WarReady { ready: true }))
        .await
        .unwrap();
    wb.send(text(ClientMsg::WarReady { ready: true }))
        .await
        .unwrap();
    let ha = tokio::spawn(async move { game_loop(&mut wa, None).await });
    let hb = tokio::spawn(async move { game_loop(&mut wb, Some(2)).await });
    (ha.await.unwrap(), hb.await.unwrap())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn clan_tournament() {
    let srv = start().await;
    let (alice, au) = srv.player("alice").await;
    let (bob, bu) = srv.player("bob").await;
    let (carol, cu) = srv.player("carol").await;
    for (tok, name, tag) in [
        (&alice, "Aquile", "AQUI"),
        (&bob, "Bisonti", "BISO"),
        (&carol, "Cervi", "CERV"),
    ] {
        assert_eq!(
            srv.post("/api/clans", Some(tok), json!({"name": name, "tag": tag}))
                .await
                .0,
            200
        );
    }

    let (st, t) = srv
        .post(
            "/api/tournaments",
            Some(&alice),
            json!({"name": "Coppa d'Autunno", "max_clans": 4}),
        )
        .await;
    assert_eq!(st, 200, "{t}");
    let tid = t["id"].as_i64().unwrap();
    assert_eq!(
        srv.post(
            &format!("/api/tournaments/{tid}/join"),
            Some(&bob),
            json!({})
        )
        .await
        .0,
        200
    );
    assert_eq!(
        srv.post(
            &format!("/api/tournaments/{tid}/start"),
            Some(&bob),
            json!({})
        )
        .await
        .0,
        403
    );
    assert_eq!(
        srv.post(
            &format!("/api/tournaments/{tid}/join"),
            Some(&carol),
            json!({})
        )
        .await
        .0,
        200
    );
    let (st, t) = srv
        .post(
            &format!("/api/tournaments/{tid}/start"),
            Some(&alice),
            json!({}),
        )
        .await;
    assert_eq!(st, 200, "{t}");
    assert_eq!(t["status"], "running");
    assert_eq!(t["rounds"].as_i64(), Some(2));
    let (dave, _) = srv.player("dave").await;
    assert_eq!(
        srv.post(
            "/api/clans",
            Some(&dave),
            json!({"name": "Delfini", "tag": "DELF"})
        )
        .await
        .0,
        200
    );
    assert_eq!(
        srv.post(
            &format!("/api/tournaments/{tid}/join"),
            Some(&dave),
            json!({})
        )
        .await
        .0,
        409
    );

    // Primo turno: la testa di serie 1 (AQUI, organizzatore) ha il turno libero.
    let m = t["matches"].as_array().unwrap();
    assert_eq!(m.len(), 2);
    let bye = m.iter().find(|x| x["bye"] == true).unwrap();
    assert_eq!(
        (bye["a"].as_str(), bye["winner"].as_str()),
        (Some("AQUI"), Some("AQUI"))
    );
    let semi = m.iter().find(|x| x["bye"] == false).unwrap();
    assert_eq!(
        (semi["a"].as_str(), semi["b"].as_str()),
        (Some("BISO"), Some("CERV"))
    );
    let semi_war = semi["war_id"].as_i64().unwrap();
    assert_eq!(semi["war_status"], "accepted");
    let (st, _) = srv
        .post(
            &format!("/api/wars/{semi_war}/cancel"),
            Some(&bob),
            json!({}),
        )
        .await;
    assert_eq!(st, 409, "gli incontri di torneo non si annullano");

    let (eb, ec) = fight(&srv, semi_war, (&bob, &bu), (&carol, &cu)).await;
    let (ServerMsg::Ended { result: rb, .. }, ServerMsg::Ended { result: rc, .. }) = (eb, ec)
    else {
        unreachable!()
    };
    assert_eq!((rb.as_str(), rc.as_str()), ("win", "loss"));

    // Finale creata automaticamente: AQUI contro BISO.
    let (_, t) = srv.get(&format!("/api/tournaments/{tid}"), None).await;
    let fin = t["matches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["round"].as_i64() == Some(2))
        .unwrap()
        .clone();
    assert_eq!(
        (fin["a"].as_str(), fin["b"].as_str()),
        (Some("AQUI"), Some("BISO"))
    );
    let final_war = fin["war_id"].as_i64().unwrap();
    let (_, before_a) = srv.get("/api/me", Some(&alice)).await;
    let (_, before_b) = srv.get("/api/me", Some(&bob)).await;
    let (ea, _) = fight(&srv, final_war, (&alice, &au), (&bob, &bu)).await;
    let ServerMsg::Ended {
        coins: war_coins, ..
    } = ea
    else {
        unreachable!()
    };

    // Torneo concluso: campioni, finalisti, premi e punti.
    let (_, t) = srv.get(&format!("/api/tournaments/{tid}"), None).await;
    assert_eq!(t["status"], "done");
    assert_eq!(t["winner"], "AQUI");
    let (_, pa) = srv.get("/api/players/alice", None).await;
    let (_, pb) = srv.get("/api/players/bob", None).await;
    assert!(
        pa["titles"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x == "Campioni del torneo «Coppa d'Autunno»")
    );
    assert!(
        pb["titles"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x == "Finalisti del torneo «Coppa d'Autunno»")
    );
    let (_, after_a) = srv.get("/api/me", Some(&alice)).await;
    let (_, after_b) = srv.get("/api/me", Some(&bob)).await;
    assert_eq!(
        after_a["coins"].as_i64().unwrap() - before_a["coins"].as_i64().unwrap(),
        war_coins + 500
    );
    assert!(after_b["coins"].as_i64().unwrap() - before_b["coins"].as_i64().unwrap() >= 200);
    let (_, c) = srv.get("/api/clans/AQUI", None).await;
    assert_eq!(
        c["season_points"].as_i64(),
        Some(5 + 15),
        "vittoria in guerra + campioni del torneo"
    );
    let (_, list) = srv.get("/api/tournaments", None).await;
    assert_eq!(list[0]["winner"], "AQUI");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn campaign_progress() {
    let srv = start().await;
    let (alice, units) = srv.player("alice").await;
    let (_, c) = srv.get("/api/campaign", Some(&alice)).await;
    assert_eq!(c["total"].as_i64(), Some(15));
    assert_eq!(c["regions"][0]["nodes"][0]["unlocked"], true);
    assert_eq!(c["regions"][0]["nodes"][1]["unlocked"], false);
    assert!(
        c["regions"][0]["outro"].is_null(),
        "la chiusura si legge solo a regione finita"
    );

    // Tappa bloccata: rifiutata.
    let mut ws = connect(&srv.ws, &alice).await;
    ws.send(text(ClientMsg::Campaign {
        node: "r1n2".into(),
        units: units.clone(),
    }))
    .await
    .unwrap();
    loop {
        match next_msg(&mut ws).await {
            ServerMsg::Error { message } => {
                assert!(message.contains("sbloccata"), "{message}");
                break;
            }
            ServerMsg::Welcome { .. } => {}
            other => panic!("inatteso: {other:?}"),
        }
    }

    // Tutorial: si ripete finché non si vince (2 nemici più deboli).
    let mut won = false;
    for _ in 0..4 {
        ws.send(text(ClientMsg::Campaign {
            node: "r1n1".into(),
            units: units.clone(),
        }))
        .await
        .unwrap();
        let (mission, biome, enemies) = loop {
            if let ServerMsg::Matched {
                mode,
                mission,
                snapshot,
                ..
            } = next_msg(&mut ws).await
            {
                assert_eq!(mode, Mode::Campaign);
                let enemies = snapshot.units.iter().filter(|u| u.team == 1).count();
                break (mission.unwrap(), snapshot.map.biome, enemies);
            }
        };
        assert!(mission.tutorial);
        assert_eq!(mission.node.as_deref(), Some("r1n1"));
        assert_eq!(biome, tactica_core::Biome::Meadow);
        assert_eq!(enemies, 2);
        let ServerMsg::Ended { result, .. } = game_loop_with(&mut ws, (0..4).collect(), None).await
        else {
            unreachable!()
        };
        if result == "win" {
            won = true;
            break;
        }
    }
    assert!(won, "il tutorial dovrebbe essere vincibile");
    let (_, c) = srv.get("/api/campaign", Some(&alice)).await;
    assert_eq!(c["cleared"].as_i64(), Some(1));
    assert_eq!(c["regions"][0]["nodes"][0]["cleared"], true);
    assert_eq!(c["regions"][0]["nodes"][1]["unlocked"], true);
    let (_, p) = srv.get("/api/players/alice", None).await;
    assert_eq!(p["recent"][0]["mode"], "campaign");
}

#[tokio::test]
async fn auth_rate_limits() {
    let s = start_with(|c| {
        c.register_per_hour = 2;
        c.auth_per_minute = 1000;
    })
    .await;
    let cred = |u: &str, p: &str| json!({"username": u, "password": p});
    // Registrazioni per IP: la terza è respinta.
    assert_eq!(
        s.post("/api/register", None, cred("primo", "password1"))
            .await
            .0,
        200
    );
    assert_eq!(
        s.post("/api/register", None, cred("secondo", "password1"))
            .await
            .0,
        200
    );
    assert_eq!(
        s.post("/api/register", None, cred("terzo", "password1"))
            .await
            .0,
        429
    );
    // Dietro il proxy (connessione da localhost) conta l'IP di X-Forwarded-For.
    let r = s
        .http
        .post(format!("{}/api/register", s.base))
        .header("x-forwarded-for", "203.0.113.7")
        .json(&cred("terzo", "password1"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status().as_u16(), 200);
    // Apache accoda il vero indirizzo a quello mandato dal client: conta l'ultimo,
    // quindi cambiare l'indirizzo inventato davanti non aggira il limite (2 all'ora).
    for (spoof, name, expect) in [
        ("198.51.100.1", "quarto", 200),
        ("198.51.100.2", "quinto", 429),
    ] {
        let r = s
            .http
            .post(format!("{}/api/register", s.base))
            .header("x-forwarded-for", format!("{spoof}, 203.0.113.7"))
            .json(&cred(name, "password1"))
            .send()
            .await
            .unwrap();
        assert_eq!(
            r.status().as_u16(),
            expect,
            "il primo indirizzo non deve contare"
        );
    }

    // Password sbagliate: dopo 5 errori l'account è bloccato, anche con la password giusta.
    for _ in 0..5 {
        assert_eq!(
            s.post("/api/login", None, cred("primo", "sbagliata"))
                .await
                .0,
            401
        );
    }
    assert_eq!(
        s.post("/api/login", None, cred("primo", "password1"))
            .await
            .0,
        429
    );
    // Gli altri account non ne risentono, e un accesso riuscito azzera il conteggio.
    assert_eq!(
        s.post("/api/login", None, cred("secondo", "sbagliata"))
            .await
            .0,
        401
    );
    assert_eq!(
        s.post("/api/login", None, cred("secondo", "password1"))
            .await
            .0,
        200
    );

    // Limite al minuto per IP.
    let s = start_with(|c| c.auth_per_minute = 3).await;
    for _ in 0..3 {
        assert_eq!(
            s.post("/api/login", None, cred("nessuno", "password1"))
                .await
                .0,
            401
        );
    }
    assert_eq!(
        s.post("/api/login", None, cred("nessuno", "password1"))
            .await
            .0,
        429
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn loot_from_defeated_units() {
    let srv = start().await;
    let (tok, units) = srv.player("razziatore").await;
    // Il bottino si trova solo raggiungendo le unità sconfitte: con qualche
    // partita contro l'IA le proprie unità ne raccolgono di sicuro.
    let mut found: Vec<Value> = Vec::new();
    for _ in 0..6 {
        let m = play(srv.ws.clone(), tok.clone(), Mode::Pve, units.clone(), None).await;
        let ServerMsg::Ended { loot, .. } = m else {
            panic!("fine partita attesa")
        };
        found.extend(loot);
        if found.len() >= 2 {
            break;
        }
    }
    assert!(!found.is_empty(), "nessun oggetto raccolto in 6 partite");
    for it in &found {
        assert!(it["name"].as_str().is_some_and(|n| !n.is_empty()));
        assert!(it["rarity"].as_str().is_some());
    }

    // Tutto ciò che è stato raccolto è nell'inventario.
    let (_, inv) = srv.get("/api/inventory", Some(&tok)).await;
    let owned = inv["items"].as_array().unwrap();
    for it in &found {
        let key = it["key"].as_str().unwrap();
        assert!(
            owned
                .iter()
                .any(|o| o["key"] == key && o["qty"].as_i64() >= Some(1)),
            "{key} manca dall'inventario"
        );
    }

    // Un oggetto trovato si può equipaggiare (se adatto a una delle unità) e vendere.
    let (_, me) = srv.get("/api/me", Some(&tok)).await;
    let roster = me["roster"].as_array().unwrap();
    'equip: for it in found.iter().filter(|i| i["slot_key"] != "bag") {
        let classes: Vec<&str> = it["classes"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|c| c.as_str())
            .collect();
        for u in roster {
            if classes.contains(&u["class"].as_str().unwrap()) {
                let id = u["id"].as_i64().unwrap();
                let mut items: Vec<Value> = u["equipment"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|k| {
                        owned
                            .iter()
                            .chain(found.iter())
                            .all(|o| o["key"] != **k || o["slot_key"] != it["slot_key"])
                    })
                    .cloned()
                    .collect();
                // Toglie l'oggetto dello stesso slot (gli oggetti iniziali sono arma e corazza).
                let slot = it["slot_key"].as_str().unwrap();
                items.retain(|k| {
                    let k = k.as_str().unwrap();
                    !((slot == "weapon" && ["iron_sword", "shortbow", "rod", "staff"].contains(&k))
                        || (slot == "armor" && k == "leather"))
                });
                items.push(it["key"].clone());
                let (st, body) = srv
                    .put(
                        &format!("/api/roster/{id}/equipment"),
                        &tok,
                        json!({ "items": items }),
                    )
                    .await;
                assert_eq!(st, 200, "{body}");
                assert!(body["equipment"].as_array().unwrap().contains(&it["key"]));
                break 'equip;
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn battle_bag_consumables() {
    let srv = start().await;
    let (tok, units) = srv.player("speziale").await;
    // Pozioni dal negozio, poi nella borsa della prima unità.
    for _ in 0..2 {
        let (st, body) = srv
            .post("/api/shop/buy", Some(&tok), json!({"item": "c.pozione.0"}))
            .await;
        assert_eq!(st, 200, "{body}");
    }
    let (st, body) = srv
        .put(
            &format!("/api/roster/{}/bag", units[0]),
            &tok,
            json!({"items": ["c.pozione.0", "c.pozione.0"]}),
        )
        .await;
    assert_eq!(st, 200, "{body}");
    assert_eq!(body["bag"].as_array().unwrap().len(), 2);
    // Equipaggiamento e borsa non si mescolano; la borsa ha due posti.
    let (st, _) = srv
        .put(
            &format!("/api/roster/{}/bag", units[1]),
            &tok,
            json!({"items": ["iron_sword"]}),
        )
        .await;
    assert_eq!(st, 400);
    let (_, inv) = srv.get("/api/inventory", Some(&tok)).await;
    assert!(
        !inv["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["key"] == "c.pozione.0"),
        "le pozioni sono nella borsa, non nell'inventario"
    );

    // Qualche partita: prima o poi l'unità beve almeno una pozione.
    let mut left = 2;
    for _ in 0..5 {
        play(srv.ws.clone(), tok.clone(), Mode::Pve, units.clone(), None).await;
        let (_, me) = srv.get("/api/me", Some(&tok)).await;
        let u = me["roster"]
            .as_array()
            .unwrap()
            .iter()
            .find(|u| u["id"] == units[0])
            .unwrap()
            .clone();
        left = u["bag"].as_array().unwrap().len();
        if left < 2 {
            break;
        }
    }
    assert!(left < 2, "nessuna pozione usata in 5 partite");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn workshop_salvage_and_brew() {
    let srv = start().await;
    let (tok, units) = srv.player("artigiano").await;
    let qty = |w: &Value, k: &str| {
        w["materials"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["key"] == k)
            .map_or(0, |m| m["qty"].as_i64().unwrap())
    };
    // Smantellare una spada comprata dà metallo.
    let (st, _) = srv
        .post("/api/shop/buy", Some(&tok), json!({"item": "iron_sword"}))
        .await;
    assert_eq!(st, 200);
    let (st, body) = srv
        .post(
            "/api/craft/salvage",
            Some(&tok),
            json!({"item": "iron_sword"}),
        )
        .await;
    assert_eq!(st, 200, "{body}");
    assert!(qty(&body["workshop"], "metal") >= 1);
    // Non si smantella ciò che non si possiede.
    let (st, _) = srv
        .post(
            "/api/craft/salvage",
            Some(&tok),
            json!({"item": "iron_sword"}),
        )
        .await;
    assert_eq!(st, 409);
    // Senza erbe l'alchimia fallisce; due partite ne danno almeno 2.
    let (st, _) = srv
        .post(
            "/api/craft/brew",
            Some(&tok),
            json!({"recipe": "pozione", "tier": 0}),
        )
        .await;
    assert_eq!(st, 409);
    for _ in 0..2 {
        play(srv.ws.clone(), tok.clone(), Mode::Pve, units.clone(), None).await;
    }
    let (_, w) = srv.get("/api/craft", Some(&tok)).await;
    assert!(qty(&w, "herbs") >= 2, "{w}");
    assert!(w["recipes"].as_array().unwrap().len() >= 10);
    let (st, body) = srv
        .post(
            "/api/craft/brew",
            Some(&tok),
            json!({"recipe": "pozione", "tier": 0}),
        )
        .await;
    assert_eq!(st, 200, "{body}");
    assert_eq!(body["item"]["key"], "c.pozione.0");
    let (_, inv) = srv.get("/api/inventory", Some(&tok)).await;
    assert!(
        inv["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["key"] == "c.pozione.0")
    );
    // Qualità oltre il limite dell'alchimia: rifiutata.
    let (st, _) = srv
        .post(
            "/api/craft/brew",
            Some(&tok),
            json!({"recipe": "pozione", "tier": 5}),
        )
        .await;
    assert_eq!(st, 400);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn recruits_choose_their_people() {
    let srv = start().await;
    let (tok, _) = srv.player("popoli").await;
    let (st, u) = srv
        .post(
            "/api/roster",
            Some(&tok),
            json!({"name": "Fronda", "class": "scout", "people": "sylvan"}),
        )
        .await;
    assert_eq!(st, 200, "{u}");
    assert_eq!(u["people"], "sylvan");
    // Esploratore (MOV 5) silvano: MOV 6.
    assert_eq!(u["mov"], 6);
    let (st, u) = srv
        .post(
            "/api/roster",
            Some(&tok),
            json!({"name": "Masso", "class": "vanguard", "people": "stonekin"}),
        )
        .await;
    assert_eq!(st, 200, "{u}");
    assert_eq!(u["mov"], 3);
    let (st, _) = srv
        .post(
            "/api/roster",
            Some(&tok),
            json!({"name": "Nessuno", "class": "scout", "people": "drago"}),
        )
        .await;
    assert_eq!(st, 400);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn admin_panel_and_history() {
    let srv = start().await;
    // "boss" è in TACTICA_ADMINS (vedi `start_with`): admin dalla registrazione.
    let (boss, _) = srv.player("boss").await;
    let (pleb, pleb_units) = srv.player("pleb").await;
    let (_, me) = srv.get("/api/me", Some(&boss)).await;
    assert_eq!(me["is_admin"], true);
    let (_, me) = srv.get("/api/me", Some(&pleb)).await;
    assert_eq!(me["is_admin"], false);
    let (st, _) = srv.get("/api/admin/overview", Some(&pleb)).await;
    assert_eq!(st, 403);
    let (st, ov) = srv.get("/api/admin/overview", Some(&boss)).await;
    assert_eq!(st, 200, "{ov}");
    assert_eq!(ov["accounts"], 2);
    assert_eq!(ov["admins"], 1);

    // Impostazioni: monete iniziali, messaggio, monete di fine partita raddoppiate.
    let (st, set) = srv
        .put(
            "/api/admin/settings",
            &boss,
            json!({"registrations_open": true, "maintenance": false, "motd": "Benvenuti!",
                   "start_coins": 1000, "coins_pct": 200, "xp_pct": 100}),
        )
        .await;
    assert_eq!(st, 200, "{set}");
    let (st, s) = srv
        .post(
            "/api/register",
            None,
            json!({"username": "nuovo", "password": "password-lunga"}),
        )
        .await;
    assert_eq!(st, 200, "{s}");
    let nuovo = s["token"].as_str().unwrap().to_string();
    let (_, me) = srv.get("/api/me", Some(&nuovo)).await;
    assert_eq!(me["coins"], 1000);
    assert_eq!(me["motd"], "Benvenuti!");

    // Una partita PvE: compare nello storico, nel riepilogo e nelle statistiche.
    let (_, before) = srv.get("/api/me", Some(&pleb)).await;
    let ended = play(
        srv.ws.clone(),
        pleb.clone(),
        Mode::Pve,
        pleb_units.clone(),
        None,
    )
    .await;
    let ServerMsg::Ended {
        match_id, coins, ..
    } = ended
    else {
        panic!()
    };
    let (_, after) = srv.get("/api/me", Some(&pleb)).await;
    assert_eq!(
        after["coins"].as_i64().unwrap() - before["coins"].as_i64().unwrap(),
        coins
    );
    assert!(
        [80, 140, 240].contains(&coins),
        "monete raddoppiate: {coins}"
    );
    let (st, h) = srv.get("/api/me/matches", Some(&pleb)).await;
    assert_eq!(st, 200, "{h}");
    assert_eq!(h["total"], 1);
    assert_eq!(h["matches"][0]["id"], match_id);
    assert_eq!(h["matches"][0]["units"].as_array().unwrap().len(), 4);
    let (st, sm) = srv
        .get(&format!("/api/matches/{match_id}/summary"), Some(&pleb))
        .await;
    assert_eq!(st, 200, "{sm}");
    assert!(sm["units"].as_array().unwrap().len() >= 8);
    let (_, ms) = srv.get("/api/me/stats", Some(&pleb)).await;
    assert_eq!(ms["played"], 1);
    assert_eq!(ms["elo"].as_array().unwrap().len(), 1);
    assert_eq!(ms["units"].as_array().unwrap().len(), 4);
    let (_, tr) = srv.get("/api/stats/trends", None).await;
    assert_eq!(tr["days"].as_array().unwrap().len(), 30);
    assert_eq!(tr["days"][29]["matches"], 1);

    // Utenti: ricerca, monete, sospensione e riattivazione.
    let (_, page) = srv.get("/api/admin/users?q=ple", Some(&boss)).await;
    assert_eq!(page["total"], 1);
    let pid = page["users"][0]["id"].as_i64().unwrap();
    let (st, c) = srv
        .post(
            &format!("/api/admin/users/{pid}/coins"),
            Some(&boss),
            json!({"delta": 500, "reason": "premio"}),
        )
        .await;
    assert_eq!(st, 200, "{c}");
    let (_, detail) = srv
        .get(&format!("/api/admin/users/{pid}"), Some(&boss))
        .await;
    assert_eq!(detail["user"]["coins"], c["coins"]);
    assert_eq!(detail["recent"].as_array().unwrap().len(), 1);
    let (st, _) = srv
        .post(
            &format!("/api/admin/users/{pid}/ban"),
            Some(&boss),
            json!({"days": 3, "reason": "linguaggio"}),
        )
        .await;
    assert_eq!(st, 200);
    let (st, _) = srv.get("/api/me", Some(&pleb)).await;
    assert_eq!(st, 401, "le sessioni del sospeso sono chiuse");
    let (st, e) = srv
        .post(
            "/api/login",
            None,
            json!({"username": "pleb", "password": "password-lunga"}),
        )
        .await;
    assert_eq!(st, 403);
    assert!(e["error"].as_str().unwrap().contains("linguaggio"), "{e}");
    let (_, banned) = srv.get("/api/admin/users?filter=banned", Some(&boss)).await;
    assert_eq!(banned["total"], 1);
    srv.post(
        &format!("/api/admin/users/{pid}/unban"),
        Some(&boss),
        json!({}),
    )
    .await;
    let (st, s) = srv
        .post(
            "/api/login",
            None,
            json!({"username": "pleb", "password": "password-lunga"}),
        )
        .await;
    assert_eq!(st, 200);
    let pleb = s["token"].as_str().unwrap().to_string();

    // Non ci si sospende da soli né ci si toglie il ruolo.
    let bid = srv.get("/api/me", Some(&boss)).await.1["id"]
        .as_i64()
        .unwrap();
    let (st, _) = srv
        .post(
            &format!("/api/admin/users/{bid}/ban"),
            Some(&boss),
            json!({}),
        )
        .await;
    assert_eq!(st, 400);
    let (st, _) = srv
        .post(
            &format!("/api/admin/users/{bid}/role"),
            Some(&boss),
            json!({"admin": false}),
        )
        .await;
    assert_eq!(st, 400);

    // Manutenzione: le partite non partono; registrazioni chiuse.
    srv.put(
        "/api/admin/settings",
        &boss,
        json!({"registrations_open": false, "maintenance": true, "motd": "", "start_coins": 400, "coins_pct": 100, "xp_pct": 100}),
    )
    .await;
    let mut ws = connect(&srv.ws, &pleb).await;
    ws.send(text(ClientMsg::Queue {
        mode: Mode::Pve,
        units: pleb_units.clone(),
    }))
    .await
    .unwrap();
    loop {
        match next_msg(&mut ws).await {
            ServerMsg::Error { message } => {
                assert!(message.contains("manutenzione"), "{message}");
                break;
            }
            ServerMsg::Matched { .. } => panic!("partita avviata in manutenzione"),
            _ => {}
        }
    }
    let (st, _) = srv
        .post(
            "/api/register",
            None,
            json!({"username": "tardi", "password": "password-lunga"}),
        )
        .await;
    assert_eq!(st, 403);
    // Connessioni aperte e chiusura forzata.
    let (_, online) = srv.get("/api/admin/online", Some(&boss)).await;
    let conn = online
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["username"] == "pleb")
        .unwrap()["conn"]
        .as_u64()
        .unwrap();
    let (st, _) = srv
        .post(
            &format!("/api/admin/online/{conn}/kick"),
            Some(&boss),
            json!({}),
        )
        .await;
    assert_eq!(st, 200);
    let closed = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match ws.next().await {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return,
                _ => {}
            }
        }
    })
    .await;
    assert!(closed.is_ok(), "la connessione doveva chiudersi");

    // Registro delle azioni.
    let (_, log) = srv.get("/api/admin/log", Some(&boss)).await;
    let actions: Vec<&str> = log
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["action"].as_str().unwrap())
        .collect();
    for a in [
        "impostazioni",
        "monete",
        "sospensione",
        "riattivazione",
        "connessione chiusa",
    ] {
        assert!(actions.contains(&a), "manca {a} in {actions:?}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn version_account_and_ws_limits() {
    let srv = start().await;
    // Versione: quella giusta passa, una diversa riceve 426.
    let (st, v) = srv.get("/api/version", None).await;
    assert_eq!(st, 200);
    let version = v["version"].as_str().unwrap().to_string();
    assert_eq!(version, tactica_core::compat_version());
    let ok = srv
        .http
        .get(format!("{}/api/leaderboard", srv.base))
        .header("X-Tactica-Version", &version)
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status().as_u16(), 200);
    let old = srv
        .http
        .get(format!("{}/api/leaderboard", srv.base))
        .header("X-Tactica-Version", "0.0")
        .send()
        .await
        .unwrap();
    assert_eq!(old.status().as_u16(), 426);
    let body: Value = old.json().await.unwrap();
    assert!(
        body["error"].as_str().unwrap().contains("ultima versione"),
        "{body}"
    );

    let (token, units) = srv.player("conto").await;
    // Statistiche di chi non ha ancora giocato: vuote, non un errore.
    let (st, ms) = srv.get("/api/me/stats", Some(&token)).await;
    assert_eq!(st, 200, "{ms}");
    assert_eq!(ms["played"], 0);
    assert_eq!(ms["units"].as_array().unwrap().len(), 4);
    let (st, h) = srv.get("/api/me/matches", Some(&token)).await;
    assert_eq!(st, 200, "{h}");
    let (st, tr) = srv.get("/api/stats/trends", None).await;
    assert_eq!(st, 200, "{tr}");
    assert!(
        connect_async(format!("{}?token={token}&v=0.0", srv.ws))
            .await
            .is_err(),
        "un client vecchio non deve aprire il WebSocket"
    );

    // Cambio della password: serve quella attuale; le vecchie sessioni si chiudono.
    let res = srv
        .http
        .put(format!("{}/api/me/password", srv.base))
        .bearer_auth(&token)
        .json(&json!({"old": "sbagliata", "new": "nuova-password"}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status().as_u16(), 403);
    let (st, s) = srv
        .put(
            "/api/me/password",
            &token,
            json!({"old": "password-lunga", "new": "nuova-password"}),
        )
        .await;
    assert_eq!(st, 200, "{s}");
    let token2 = s["token"].as_str().unwrap().to_string();
    assert_eq!(srv.get("/api/me", Some(&token)).await.0, 401);
    assert_eq!(srv.get("/api/me", Some(&token2)).await.0, 200);
    let (st, _) = srv
        .post(
            "/api/login",
            None,
            json!({"username": "conto", "password": "nuova-password"}),
        )
        .await;
    assert_eq!(st, 200);

    // Troppi messaggi: avviso e poi chiusura della connessione.
    let mut ws = connect(&srv.ws, &token2).await;
    for _ in 0..200 {
        if ws.send(text(ClientMsg::Cancel)).await.is_err() {
            break;
        }
    }
    let mut warned = false;
    let closed = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            match ws.next().await {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return,
                Some(Ok(Message::Text(t))) => {
                    if t.as_str().contains("troppi messaggi") {
                        warned = true;
                    }
                }
                _ => {}
            }
        }
    })
    .await;
    assert!(closed.is_ok() && warned, "avviso e chiusura attesi");
    // Un messaggio enorme chiude la connessione.
    let mut ws = connect(&srv.ws, &token2).await;
    let _ = ws.send(Message::text("x".repeat(40_000))).await;
    let closed = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match ws.next().await {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return,
                _ => {}
            }
        }
    })
    .await;
    assert!(
        closed.is_ok(),
        "messaggio troppo grande: connessione chiusa"
    );

    // Cancellazione dell'account: il clan rimasto vuoto si scioglie.
    let (st, c) = srv
        .post(
            "/api/clans",
            Some(&token2),
            json!({"name": "Solitari", "tag": "SOLO", "motto": ""}),
        )
        .await;
    assert_eq!(st, 200, "{c}");
    let res = srv
        .http
        .delete(format!("{}/api/me", srv.base))
        .bearer_auth(&token2)
        .json(&json!({"password": "sbagliata"}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status().as_u16(), 403);
    let res = srv
        .http
        .delete(format!("{}/api/me", srv.base))
        .bearer_auth(&token2)
        .json(&json!({"password": "nuova-password"}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status().as_u16(), 200);
    let (st, _) = srv
        .post(
            "/api/login",
            None,
            json!({"username": "conto", "password": "nuova-password"}),
        )
        .await;
    assert_eq!(st, 401);
    let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM roster WHERE id = ?")
        .bind(units[0])
        .fetch_one(&srv.db)
        .await
        .unwrap();
    assert_eq!(left, 0);
    let (st, _) = srv.get("/api/clans/SOLO", None).await;
    assert_eq!(st, 404);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tournament_deadlines_and_admin() {
    let srv = start().await;
    let (boss, _) = srv.player("boss").await;
    let mut players = Vec::new();
    for (name, clan, tag) in [
        ("anna", "Aironi", "AIRO"),
        ("bruno", "Bufali", "BUFA"),
        ("ciro", "Cigni", "CIGN"),
        ("dora", "Daini", "DAIN"),
    ] {
        let (tok, units) = srv.player(name).await;
        assert_eq!(
            srv.post("/api/clans", Some(&tok), json!({"name": clan, "tag": tag}))
                .await
                .0,
            200
        );
        players.push((tok, units, tag));
    }
    let (st, t) = srv
        .post(
            "/api/tournaments",
            Some(&players[0].0),
            json!({"name": "Coppa delle Scadenze", "max_clans": 4}),
        )
        .await;
    assert_eq!(st, 200, "{t}");
    let tid = t["id"].as_i64().unwrap();
    for p in &players[1..] {
        assert_eq!(
            srv.post(
                &format!("/api/tournaments/{tid}/join"),
                Some(&p.0),
                json!({})
            )
            .await
            .0,
            200
        );
    }
    let (st, t) = srv
        .post(
            &format!("/api/tournaments/{tid}/start"),
            Some(&players[0].0),
            json!({}),
        )
        .await;
    assert_eq!(st, 200, "{t}");
    let m0 = t["matches"][0].clone();
    assert!(m0["deadline"].as_i64().unwrap() > tactica_server::db::now() + 71 * 3600);

    // Incontro 0 scaduto: si presenta solo il clan B dell'incontro, che vince a tavolino.
    let tag_b = m0["b"].as_str().unwrap().to_string();
    let p_b = players.iter().find(|p| p.2 == tag_b).unwrap();
    let war = m0["war_id"].as_i64().unwrap();
    let mut ws = connect(&srv.ws, &p_b.0).await;
    ws.send(text(ClientMsg::WarJoin {
        war,
        units: p_b.1.clone(),
    }))
    .await
    .unwrap();
    loop {
        if let ServerMsg::WarRoom { .. } = next_msg(&mut ws).await {
            break;
        }
    }
    sqlx::query("UPDATE tournament_matches SET deadline = 1 WHERE tournament_id = ? AND round = 1 AND slot = 0")
        .bind(tid)
        .execute(&srv.db)
        .await
        .unwrap();
    let (st, _) = srv
        .post(
            "/api/admin/tournaments/expire",
            Some(&players[0].0),
            json!({}),
        )
        .await;
    assert_eq!(st, 403, "solo gli admin");
    let (st, e) = srv
        .post("/api/admin/tournaments/expire", Some(&boss), json!({}))
        .await;
    assert_eq!(st, 200, "{e}");
    assert_eq!(e["decided"], 1);
    let (_, t) = srv.get(&format!("/api/tournaments/{tid}"), None).await;
    assert_eq!(t["matches"][0]["winner"].as_str(), Some(tag_b.as_str()));
    assert_eq!(t["matches"][0]["decided_by"], "forfeit");
    assert_eq!(t["matches"][0]["war_status"], "cancelled");

    // Incontro 1 assegnato dall'admin: si crea la finale.
    let m1 = t["matches"][1].clone();
    let tag_a1 = m1["a"].as_str().unwrap().to_string();
    let (st, r) = srv
        .post(
            &format!("/api/admin/tournaments/{tid}/matches/1/1/winner"),
            Some(&boss),
            json!({"tag": "XXXX"}),
        )
        .await;
    assert_eq!(st, 400, "{r}");
    let (st, r) = srv
        .post(
            &format!("/api/admin/tournaments/{tid}/matches/1/1/winner"),
            Some(&boss),
            json!({"tag": tag_a1}),
        )
        .await;
    assert_eq!(st, 200, "{r}");
    let (_, t) = srv.get(&format!("/api/tournaments/{tid}"), None).await;
    let fin = t["matches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["round"] == 2)
        .unwrap()
        .clone();
    assert_eq!(t["matches"][1]["decided_by"], "admin");
    let before = fin["deadline"].as_i64().unwrap();

    // Proroga e annullamento.
    let (st, _) = srv
        .post(
            &format!("/api/admin/tournaments/{tid}/extend"),
            Some(&boss),
            json!({"hours": 24}),
        )
        .await;
    assert_eq!(st, 200);
    let (_, t) = srv.get(&format!("/api/tournaments/{tid}"), None).await;
    let fin = t["matches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["round"] == 2)
        .unwrap()
        .clone();
    assert_eq!(fin["deadline"].as_i64().unwrap(), before + 24 * 3600);
    let (_, list) = srv.get("/api/admin/tournaments", Some(&boss)).await;
    assert_eq!(list[0]["pending"].as_array().unwrap().len(), 1);
    let (st, _) = srv
        .post(
            &format!("/api/admin/tournaments/{tid}/cancel"),
            Some(&boss),
            json!({}),
        )
        .await;
    assert_eq!(st, 200);
    let (_, t) = srv.get(&format!("/api/tournaments/{tid}"), None).await;
    assert_eq!(t["status"], "cancelled");
    let fin_war = fin["war_id"].as_i64().unwrap();
    let st: String = sqlx::query_scalar("SELECT status FROM clan_wars WHERE id = ?")
        .bind(fin_war)
        .fetch_one(&srv.db)
        .await
        .unwrap();
    assert_eq!(st, "cancelled");

    // Torneo a due senza nessuno schierato: passa il clan con l'ELO più alto e il torneo si chiude.
    sqlx::query("UPDATE clans SET rating = 1200 WHERE tag = 'DAIN'")
        .execute(&srv.db)
        .await
        .unwrap();
    let (_, t2) = srv
        .post(
            "/api/tournaments",
            Some(&players[2].0),
            json!({"name": "Sfida a due", "max_clans": 2}),
        )
        .await;
    let t2 = t2["id"].as_i64().unwrap();
    srv.post(
        &format!("/api/tournaments/{t2}/join"),
        Some(&players[3].0),
        json!({}),
    )
    .await;
    let (st, _) = srv
        .post(
            &format!("/api/tournaments/{t2}/start"),
            Some(&players[2].0),
            json!({}),
        )
        .await;
    assert_eq!(st, 200);
    sqlx::query("UPDATE tournament_matches SET deadline = 1 WHERE tournament_id = ?")
        .bind(t2)
        .execute(&srv.db)
        .await
        .unwrap();
    srv.post("/api/admin/tournaments/expire", Some(&boss), json!({}))
        .await;
    let (_, t) = srv.get(&format!("/api/tournaments/{t2}"), None).await;
    assert_eq!(t["status"], "done");
    assert_eq!(t["winner"], "DAIN");
}

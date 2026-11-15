//! Campagna in singolo: mappa del mondo a nodi con una storia originale.
//!
//! Cinque regioni, ognuna con il proprio bioma e tre tappe; l'ultima tappa di
//! ogni regione è la caccia a un campione. La prima tappa è il tutorial.
//! Una tappa si sblocca quando è stata vinta la precedente.

use axum::Json;
use axum::extract::State;
use serde::Serialize;
use sqlx::SqliteConnection;
use tactica_core::Biome;

use crate::AppState;
use crate::auth::AuthUser;
use crate::db::now;
use crate::error::ApiResult;

pub struct NodeDef {
    pub key: &'static str,
    pub name: &'static str,
    pub story: &'static str,
    /// Livelli aggiunti ai nemici rispetto alla media della squadra.
    pub level_bonus: i32,
    pub difficulty: u8,
    pub champion: bool,
    pub tutorial: bool,
    /// Posizione sulla mappa (0..1).
    pub x: f32,
    pub y: f32,
}

pub struct RegionDef {
    pub name: &'static str,
    pub biome: Biome,
    pub intro: &'static str,
    pub outro: &'static str,
    pub nodes: [NodeDef; 3],
}

macro_rules! node {
    ($key:literal, $name:literal, $story:literal, $lv:literal, $d:literal, $champ:literal, $tut:literal, $x:literal, $y:literal) => {
        NodeDef {
            key: $key,
            name: $name,
            story: $story,
            level_bonus: $lv,
            difficulty: $d,
            champion: $champ,
            tutorial: $tut,
            x: $x,
            y: $y,
        }
    };
}

pub static REGIONS: [RegionDef; 5] = [
    RegionDef {
        name: "Piana di Lunaverde",
        biome: Biome::Meadow,
        intro: "Nelle Terre di Velmara ogni contesa si risolve secondo il Patto: prima di ogni scontro i Custodi estraggono gli Statuti, e chi li infrange viene sospeso o cacciato dal campo. Da qualche mese, però, bande senza insegne combattono ignorando il Patto. La tua compagnia parte da Lunaverde per capire chi le guida.",
        outro: "Il capobanda sconfitto porta al collo un sigillo spezzato: il simbolo dei Custodi, inciso al rovescio. Qualcuno sta corrompendo il Patto dall'interno.",
        nodes: [
            node!(
                "r1n1",
                "Prima prova",
                "Un vecchio Custode ti mette alla prova sui campi di Lunaverde: impara a muoverti, a colpire e a rispettare gli Statuti.",
                -1,
                1,
                false,
                true,
                0.07,
                0.62
            ),
            node!(
                "r1n2",
                "Mulino assediato",
                "Predoni senza insegne hanno occupato il mulino. I contadini chiedono aiuto.",
                0,
                1,
                false,
                false,
                0.15,
                0.45
            ),
            node!(
                "r1n3",
                "Il capobanda",
                "Il capo dei predoni si è trincerato sulla collina. Abbattilo e la banda si disperderà.",
                1,
                1,
                true,
                false,
                0.23,
                0.6
            ),
        ],
    },
    RegionDef {
        name: "Acquitrini di Nebbiaforte",
        biome: Biome::Marsh,
        intro: "Il sigillo spezzato porta a Nebbiaforte, dove le paludi nascondono un vecchio tempio dei Custodi. Si dice che lì le Tavole del Patto venissero copiate e mandate in tutte le contee.",
        outro: "Nel tempio le Tavole sono state riscritte: alcuni Statuti favoriscono sempre la stessa parte. La mano che le ha alterate ha lasciato una firma: «la Cenere».",
        nodes: [
            node!(
                "r2n1",
                "Guado delle canne",
                "Il guado è sorvegliato da arcieri nascosti tra le canne. Avanza con prudenza.",
                1,
                1,
                false,
                false,
                0.32,
                0.38
            ),
            node!(
                "r2n2",
                "Il villaggio sommerso",
                "Un villaggio mezzo sommerso fa da rifugio a chi altera gli Statuti.",
                2,
                2,
                false,
                false,
                0.4,
                0.55
            ),
            node!(
                "r2n3",
                "La guardiana del tempio",
                "Una guardiana custodisce le Tavole contraffatte e non lascia passare nessuno.",
                3,
                2,
                true,
                false,
                0.47,
                0.4
            ),
        ],
    },
    RegionDef {
        name: "Distese di Sabbiarossa",
        biome: Biome::Desert,
        intro: "La Cenere ha seguaci anche nel deserto di Sabbiarossa, dove le carovane pagano pedaggi a chi controlla i pozzi. Senza acqua, nessuno può attraversare le Distese.",
        outro: "Liberati i pozzi, una carovaniera ti mostra una mappa: la Cenere ha la sua roccaforte tra i ghiacci del nord.",
        nodes: [
            node!(
                "r3n1",
                "Il pozzo conteso",
                "Due fazioni si contendono il pozzo più grande. Nessuna delle due rispetta gli Statuti.",
                3,
                2,
                false,
                false,
                0.56,
                0.6
            ),
            node!(
                "r3n2",
                "Rovine nella sabbia",
                "Tra le rovine si nasconde un deposito di armi della Cenere.",
                4,
                2,
                false,
                false,
                0.63,
                0.45
            ),
            node!(
                "r3n3",
                "Il signore dei pozzi",
                "Il signore dei pozzi esige un tributo da chiunque passi. È ora che paghi lui.",
                5,
                2,
                true,
                false,
                0.7,
                0.62
            ),
        ],
    },
    RegionDef {
        name: "Bocca di Brace",
        biome: Biome::Volcano,
        intro: "Per raggiungere il nord bisogna attraversare la Bocca di Brace, una valle di lava dove la Cenere forgia le proprie armi.",
        outro: "Le forge si spengono una dopo l'altra. Tra le carte del fabbro capo c'è un nome: Varen, un tempo il più stimato dei Custodi.",
        nodes: [
            node!(
                "r4n1",
                "Il ponte di ossidiana",
                "Un ponte di ossidiana sovrasta la lava: è l'unica via, ed è sorvegliato.",
                5,
                2,
                false,
                false,
                0.76,
                0.42
            ),
            node!(
                "r4n2",
                "Le forge",
                "Nelle forge si prepara un esercito. Fermale prima che sia troppo tardi.",
                6,
                3,
                false,
                false,
                0.82,
                0.58
            ),
            node!(
                "r4n3",
                "Il fabbro della Cenere",
                "Il fabbro capo difende le sue fornaci con tutto ciò che ha forgiato.",
                7,
                3,
                true,
                false,
                0.87,
                0.4
            ),
        ],
    },
    RegionDef {
        name: "Altopiano dei Custodi",
        biome: Biome::Highlands,
        intro: "Sull'altopiano sorge la Sala dei Custodi. Varen vi si è chiuso con le Tavole originali: vuole un Patto nuovo, scritto solo da lui.",
        outro: "Varen cade, e con lui la Cenere. Le Tavole originali tornano alla luce e i Custodi giurano di nuovo davanti a tutti. Il Patto è salvo: d'ora in poi saranno le compagnie come la tua a vegliare che venga rispettato.",
        nodes: [
            node!(
                "r5n1",
                "La salita",
                "La salita verso la Sala è lunga e scoperta: ogni passo è sotto tiro.",
                7,
                3,
                false,
                false,
                0.9,
                0.62
            ),
            node!(
                "r5n2",
                "I cancelli",
                "Gli ultimi fedeli di Varen difendono i cancelli della Sala.",
                8,
                3,
                false,
                false,
                0.94,
                0.47
            ),
            node!(
                "r5n3",
                "Varen",
                "Varen ti attende davanti alle Tavole. Il destino del Patto si decide qui.",
                9,
                3,
                true,
                false,
                0.97,
                0.3
            ),
        ],
    },
];

pub fn find(key: &str) -> Option<(usize, usize)> {
    REGIONS
        .iter()
        .enumerate()
        .find_map(|(r, reg)| reg.nodes.iter().position(|n| n.key == key).map(|i| (r, i)))
}

/// Chiave della tappa precedente (`None` per la prima).
pub fn previous(key: &str) -> Option<&'static str> {
    let (r, i) = find(key)?;
    if i > 0 {
        Some(REGIONS[r].nodes[i - 1].key)
    } else if r > 0 {
        Some(REGIONS[r - 1].nodes[2].key)
    } else {
        None
    }
}

pub async fn cleared(conn: &mut SqliteConnection, account: i64) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar("SELECT node FROM campaign_progress WHERE account_id = ?")
        .bind(account)
        .fetch_all(conn)
        .await
}

pub async fn unlocked(conn: &mut SqliteConnection, account: i64, key: &str) -> sqlx::Result<bool> {
    match previous(key) {
        None => Ok(find(key).is_some()),
        Some(p) => Ok(cleared(conn, account).await?.iter().any(|c| c == p)),
    }
}

pub async fn record_clear(
    conn: &mut SqliteConnection,
    account: i64,
    key: &str,
    turns: i64,
) -> sqlx::Result<bool> {
    let n = sqlx::query(
        "INSERT INTO campaign_progress (account_id, node, cleared_at, turns) VALUES (?, ?, ?, ?)
         ON CONFLICT(account_id, node) DO UPDATE SET turns = MIN(turns, excluded.turns)",
    )
    .bind(account)
    .bind(key)
    .bind(now())
    .bind(turns)
    .execute(conn)
    .await?
    .rows_affected();
    Ok(n > 0)
}

#[derive(Serialize)]
pub struct NodeView {
    key: &'static str,
    name: &'static str,
    story: &'static str,
    champion: bool,
    tutorial: bool,
    difficulty: u8,
    x: f32,
    y: f32,
    cleared: bool,
    unlocked: bool,
}

#[derive(Serialize)]
pub struct RegionView {
    name: &'static str,
    biome: &'static str,
    intro: &'static str,
    /// La chiusura si legge solo dopo aver vinto l'ultima tappa della regione.
    outro: Option<&'static str>,
    nodes: Vec<NodeView>,
}

#[derive(Serialize)]
pub struct CampaignView {
    regions: Vec<RegionView>,
    cleared: usize,
    total: usize,
}

pub async fn view(State(s): State<AppState>, user: AuthUser) -> ApiResult<Json<CampaignView>> {
    let mut conn = s.db.acquire().await?;
    let done = cleared(&mut conn, user.id).await?;
    let is_done = |k: &str| done.iter().any(|c| c == k);
    let mut regions = Vec::new();
    for reg in &REGIONS {
        let nodes: Vec<NodeView> = reg
            .nodes
            .iter()
            .map(|n| NodeView {
                key: n.key,
                name: n.name,
                story: n.story,
                champion: n.champion,
                tutorial: n.tutorial,
                difficulty: n.difficulty,
                x: n.x,
                y: n.y,
                cleared: is_done(n.key),
                unlocked: previous(n.key).is_none_or(&is_done),
            })
            .collect();
        regions.push(RegionView {
            name: reg.name,
            biome: reg.biome.name(),
            intro: reg.intro,
            outro: is_done(reg.nodes[2].key).then_some(reg.outro),
            nodes,
        });
    }
    let total = REGIONS.len() * 3;
    Ok(Json(CampaignView {
        cleared: done.len(),
        total,
        regions,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_is_linear() {
        assert_eq!(previous("r1n1"), None);
        assert_eq!(previous("r1n2"), Some("r1n1"));
        assert_eq!(previous("r2n1"), Some("r1n3"));
        assert_eq!(previous("r5n3"), Some("r5n2"));
        assert!(find("r9n9").is_none());
        assert!(REGIONS.iter().all(|r| r.nodes[2].champion));
    }
}

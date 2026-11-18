//! Bottega: materiali, smantellamento, forgia (raffinare, incantare) e alchimia.

use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use tactica_core::craft::{self, Bundle, Cost, MAX_BREW_TIER, Material, RECIPES, Recipe};
use tactica_core::gear::ItemRef;
use tactica_core::rng::Rng;

use crate::AppState;
use crate::armory::{ItemInfo, item_info};
use crate::auth::AuthUser;
use crate::db;
use crate::error::{ApiError, ApiResult};

#[derive(Serialize)]
pub struct MaterialView {
    key: &'static str,
    name: &'static str,
    qty: i64,
}

#[derive(Serialize)]
pub struct CostView {
    materials: Vec<MaterialView>,
    coins: i32,
}

fn cost_view(c: &Cost) -> CostView {
    CostView {
        materials: c
            .materials
            .iter()
            .map(|&(m, n)| MaterialView {
                key: m.key(),
                name: m.name(),
                qty: n as i64,
            })
            .collect(),
        coins: c.coins,
    }
}

#[derive(Serialize)]
pub struct RecipeView {
    code: &'static str,
    /// Per qualità I-III: prodotto e costo.
    tiers: Vec<(ItemInfo, CostView)>,
}

#[derive(Serialize)]
pub struct Workshop {
    coins: i64,
    materials: Vec<MaterialView>,
    recipes: Vec<RecipeView>,
}

pub async fn materials(
    conn: &mut SqliteConnection,
    account: i64,
) -> sqlx::Result<Vec<(Material, i64)>> {
    let rows: Vec<(String, i64)> =
        sqlx::query_as("SELECT material, qty FROM materials WHERE account_id = ?")
            .bind(account)
            .fetch_all(&mut *conn)
            .await?;
    Ok(Material::ALL
        .into_iter()
        .map(|m| {
            let q = rows.iter().find(|r| r.0 == m.key()).map_or(0, |r| r.1);
            (m, q)
        })
        .collect())
}

pub async fn add_materials(
    conn: &mut SqliteConnection,
    account: i64,
    b: &Bundle,
) -> sqlx::Result<()> {
    for &(m, n) in b {
        sqlx::query(
            "INSERT INTO materials (account_id, material, qty) VALUES (?, ?, ?)
             ON CONFLICT(account_id, material) DO UPDATE SET qty = qty + excluded.qty",
        )
        .bind(account)
        .bind(m.key())
        .bind(n as i64)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// Paga un costo (materiali e monete); `false` se non basta.
async fn pay(conn: &mut SqliteConnection, account: i64, c: &Cost) -> sqlx::Result<bool> {
    for &(m, n) in &c.materials {
        let done = sqlx::query(
            "UPDATE materials SET qty = qty - ? WHERE account_id = ? AND material = ? AND qty >= ?",
        )
        .bind(n as i64)
        .bind(account)
        .bind(m.key())
        .bind(n as i64)
        .execute(&mut *conn)
        .await?
        .rows_affected();
        if done == 0 {
            return Ok(false);
        }
    }
    let paid = sqlx::query("UPDATE accounts SET coins = coins - ? WHERE id = ? AND coins >= ?")
        .bind(c.coins as i64)
        .bind(account)
        .bind(c.coins as i64)
        .execute(&mut *conn)
        .await?
        .rows_affected();
    Ok(paid == 1)
}

async fn workshop(s: &AppState, account: i64) -> ApiResult<Workshop> {
    let mut conn = s.db.acquire().await?;
    let coins: i64 = sqlx::query_scalar("SELECT coins FROM accounts WHERE id = ?")
        .bind(account)
        .fetch_one(&mut *conn)
        .await?;
    let mats = materials(&mut conn, account).await?;
    Ok(Workshop {
        coins,
        materials: mats
            .into_iter()
            .map(|(m, q)| MaterialView {
                key: m.key(),
                name: m.name(),
                qty: q,
            })
            .collect(),
        recipes: RECIPES
            .iter()
            .map(|r| RecipeView {
                code: r.code,
                tiers: (0..=MAX_BREW_TIER)
                    .filter_map(|t| r.product(t).map(|p| (item_info(p), cost_view(&r.cost(t)))))
                    .collect(),
            })
            .collect(),
    })
}

pub async fn view(State(s): State<AppState>, user: AuthUser) -> ApiResult<Json<Workshop>> {
    Ok(Json(workshop(&s, user.id).await?))
}

#[derive(Deserialize)]
pub struct ItemReq {
    item: String,
}

fn parse(key: &str) -> ApiResult<ItemRef> {
    ItemRef::from_key(key)
        .ok_or_else(|| ApiError::bad_request(format!("oggetto sconosciuto: {key}")))
}

#[derive(Serialize)]
pub struct Crafted {
    /// Oggetto prodotto (assente per lo smantellamento).
    item: Option<ItemInfo>,
    /// Materiali ottenuti (smantellamento).
    gained: Vec<MaterialView>,
    workshop: Workshop,
}

/// Smantella un oggetto dell'inventario (non equipaggiato).
pub async fn salvage(
    State(s): State<AppState>,
    user: AuthUser,
    Json(r): Json<ItemReq>,
) -> ApiResult<Json<Crafted>> {
    let item = parse(&r.item)?;
    let gained = craft::salvage(item);
    let mut tx = s.db.begin().await?;
    if !db::take_item(&mut tx, user.id, item).await? {
        return Err(ApiError::conflict("oggetto non posseduto (o equipaggiato)"));
    }
    add_materials(&mut tx, user.id, &gained).await?;
    tx.commit().await?;
    crate::achievements::record(&s.db, user.id, &[("craft:salvage", 1)]).await;
    Ok(Json(Crafted {
        item: None,
        gained: cost_view(&Cost {
            materials: gained,
            coins: 0,
        })
        .materials,
        workshop: workshop(&s, user.id).await?,
    }))
}

async fn upgrade(
    s: &AppState,
    account: i64,
    item: ItemRef,
    cost: Option<Cost>,
    result: Option<ItemRef>,
    what: &str,
) -> ApiResult<Json<Crafted>> {
    let (Some(cost), Some(result)) = (cost, result) else {
        return Err(ApiError::bad_request(format!(
            "questo oggetto non si può {what}"
        )));
    };
    let mut tx = s.db.begin().await?;
    if !db::take_item(&mut tx, account, item).await? {
        return Err(ApiError::conflict("oggetto non posseduto (o equipaggiato)"));
    }
    if !pay(&mut tx, account, &cost).await? {
        return Err(ApiError::conflict("materiali o monete insufficienti"));
    }
    db::add_item(&mut tx, account, result, 1).await?;
    tx.commit().await?;
    Ok(Json(Crafted {
        item: Some(item_info(result)),
        gained: Vec::new(),
        workshop: workshop(s, account).await?,
    }))
}

/// Forgia: qualità del materiale +1.
pub async fn refine(
    State(s): State<AppState>,
    user: AuthUser,
    Json(r): Json<ItemReq>,
) -> ApiResult<Json<Crafted>> {
    let item = parse(&r.item)?;
    let done = upgrade(
        &s,
        user.id,
        item,
        craft::refine_cost(item),
        craft::refine(item),
        "raffinare",
    )
    .await?;
    crate::achievements::record(&s.db, user.id, &[("craft:refine", 1)]).await;
    Ok(done)
}

/// Forgia: rarità +1 con un affisso in più.
pub async fn enchant(
    State(s): State<AppState>,
    user: AuthUser,
    Json(r): Json<ItemReq>,
) -> ApiResult<Json<Crafted>> {
    let item = parse(&r.item)?;
    let mut rng = Rng::new(crate::game::random_seed());
    let result = craft::enchant(item, &mut rng);
    let done = upgrade(
        &s,
        user.id,
        item,
        craft::enchant_cost(item),
        result,
        "incantare",
    )
    .await?;
    crate::achievements::record(&s.db, user.id, &[("craft:enchant", 1)]).await;
    Ok(done)
}

#[derive(Deserialize)]
pub struct BrewReq {
    recipe: String,
    /// Qualità: 0 = I, fino a `MAX_BREW_TIER`.
    #[serde(default)]
    tier: u8,
}

/// Alchimia: produce un consumabile da una ricetta.
pub async fn brew(
    State(s): State<AppState>,
    user: AuthUser,
    Json(r): Json<BrewReq>,
) -> ApiResult<Json<Crafted>> {
    let recipe =
        Recipe::find(&r.recipe).ok_or_else(|| ApiError::bad_request("ricetta sconosciuta"))?;
    if r.tier > MAX_BREW_TIER {
        return Err(ApiError::bad_request("qualità troppo alta per l'alchimia"));
    }
    let product = recipe
        .product(r.tier)
        .ok_or_else(|| ApiError::internal("ricetta senza prodotto"))?;
    let mut tx = s.db.begin().await?;
    if !pay(&mut tx, user.id, &recipe.cost(r.tier)).await? {
        return Err(ApiError::conflict("materiali o monete insufficienti"));
    }
    db::add_item(&mut tx, user.id, product, 1).await?;
    tx.commit().await?;
    crate::achievements::record(&s.db, user.id, &[("craft:brew", 1)]).await;
    Ok(Json(Crafted {
        item: Some(item_info(product)),
        gained: Vec::new(),
        workshop: workshop(&s, user.id).await?,
    }))
}

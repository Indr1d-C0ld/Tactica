//! Cataloghi, inventario, negozio, equipaggiamento e cambio di classe.

use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};
use tactica_core::ability::AbilityId;
use tactica_core::class::{Class, Stats};
use tactica_core::item::{ItemId, validate_loadout};

use crate::AppState;
use crate::auth::AuthUser;
use crate::db::{self, RosterUnit, items_json};
use crate::error::{ApiError, ApiResult};

#[derive(Serialize)]
pub struct AbilityInfo {
    key: &'static str,
    name: &'static str,
    kind: &'static str,
    element: Option<&'static str>,
    power: i32,
    mp_cost: i32,
    range: (i32, i32),
    aoe: i32,
    master_rp: i32,
    taught_by: Vec<&'static str>,
}

fn ability_info(a: AbilityId) -> AbilityInfo {
    use tactica_core::ability::Kind;
    let d = a.def();
    AbilityInfo {
        key: d.key,
        name: d.name,
        kind: match d.kind {
            Kind::Physical => "fisica",
            Kind::Magic => "magica",
            Kind::Heal => "cura",
        },
        element: d.element.map(|e| e.name()),
        power: d.power,
        mp_cost: d.mp_cost,
        range: (d.range_min, d.range_max),
        aoe: d.aoe,
        master_rp: d.master_rp,
        taught_by: ItemId::ALL
            .into_iter()
            .filter(|i| i.def().teaches == Some(a))
            .map(ItemId::key)
            .collect(),
    }
}

#[derive(Serialize)]
pub struct ClassInfo {
    key: &'static str,
    name: &'static str,
    base: bool,
    requirement: String,
    stats_lv1: Stats,
    innate: Vec<AbilityInfo>,
    learnable: Vec<AbilityInfo>,
}

pub async fn classes() -> Json<Vec<ClassInfo>> {
    Json(
        Class::ALL
            .into_iter()
            .map(|c| ClassInfo {
                key: c.key(),
                name: c.name(),
                base: c.is_base(),
                requirement: c.requirement_text(),
                stats_lv1: c.stats_at(1),
                innate: std::iter::once(AbilityId::Strike)
                    .chain(c.innate().iter().copied())
                    .map(ability_info)
                    .collect(),
                learnable: c.learnable().iter().copied().map(ability_info).collect(),
            })
            .collect(),
    )
}

#[derive(Serialize)]
pub struct ItemInfo {
    key: &'static str,
    name: &'static str,
    slot: &'static str,
    classes: Vec<&'static str>,
    bonus: Stats,
    teaches: Option<&'static str>,
    price: i32,
    in_shop: bool,
}

fn item_info(i: ItemId) -> ItemInfo {
    let d = i.def();
    ItemInfo {
        key: d.key,
        name: d.name,
        slot: d.slot.name(),
        classes: if d.classes.is_empty() {
            Class::ALL.iter().map(|c| c.key()).collect()
        } else {
            d.classes.iter().map(|c| c.key()).collect()
        },
        bonus: d.bonus,
        teaches: d.teaches.map(AbilityId::key),
        price: d.price,
        in_shop: d.in_shop,
    }
}

pub async fn items() -> Json<Vec<ItemInfo>> {
    Json(ItemId::ALL.into_iter().map(item_info).collect())
}

#[derive(Serialize)]
pub struct Owned {
    key: String,
    name: &'static str,
    qty: i64,
}

#[derive(Serialize)]
pub struct Inventory {
    coins: i64,
    items: Vec<Owned>,
}

async fn load_inventory(s: &AppState, account: i64) -> ApiResult<Inventory> {
    let coins = sqlx::query_scalar("SELECT coins FROM accounts WHERE id = ?")
        .bind(account)
        .fetch_one(&s.db)
        .await?;
    let items = db::inventory(&s.db, account)
        .await?
        .into_iter()
        .filter_map(|(key, qty)| {
            ItemId::from_key(&key).map(|i| Owned {
                name: i.def().name,
                key,
                qty,
            })
        })
        .collect();
    Ok(Inventory { coins, items })
}

pub async fn inventory(State(s): State<AppState>, user: AuthUser) -> ApiResult<Json<Inventory>> {
    Ok(Json(load_inventory(&s, user.id).await?))
}

#[derive(Deserialize)]
pub struct Trade {
    item: String,
}

fn parse_item(key: &str) -> ApiResult<ItemId> {
    ItemId::from_key(key)
        .ok_or_else(|| ApiError::bad_request(format!("oggetto sconosciuto: {key}")))
}

pub async fn buy(
    State(s): State<AppState>,
    user: AuthUser,
    Json(t): Json<Trade>,
) -> ApiResult<Json<Inventory>> {
    let item = parse_item(&t.item)?;
    let d = item.def();
    if !d.in_shop {
        return Err(ApiError::bad_request("oggetto non in vendita"));
    }
    let mut tx = s.db.begin().await?;
    let paid = sqlx::query("UPDATE accounts SET coins = coins - ? WHERE id = ? AND coins >= ?")
        .bind(d.price as i64)
        .bind(user.id)
        .bind(d.price as i64)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if paid == 0 {
        return Err(ApiError::conflict("monete insufficienti"));
    }
    db::add_item(&mut tx, user.id, item, 1).await?;
    tx.commit().await?;
    Ok(Json(load_inventory(&s, user.id).await?))
}

pub async fn sell(
    State(s): State<AppState>,
    user: AuthUser,
    Json(t): Json<Trade>,
) -> ApiResult<Json<Inventory>> {
    let item = parse_item(&t.item)?;
    let mut tx = s.db.begin().await?;
    if !db::take_item(&mut tx, user.id, item).await? {
        return Err(ApiError::conflict("oggetto non posseduto (o equipaggiato)"));
    }
    sqlx::query("UPDATE accounts SET coins = coins + ? WHERE id = ?")
        .bind(item.def().price as i64 / 2)
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(load_inventory(&s, user.id).await?))
}

fn ensure_idle(s: &AppState, account: i64) -> ApiResult<()> {
    if s.busy.lock().unwrap().contains(&account) {
        return Err(ApiError::conflict("non durante una partita"));
    }
    Ok(())
}

async fn owned_unit(s: &AppState, account: i64, id: i64) -> ApiResult<RosterUnit> {
    db::roster_unit(&s.db, account, id)
        .await?
        .ok_or_else(|| ApiError::not_found("unità non trovata"))
}

#[derive(Deserialize)]
pub struct Equip {
    /// Configurazione completa desiderata (chiavi oggetto, max una per slot).
    items: Vec<String>,
}

/// Sostituisce l'equipaggiamento: i vecchi oggetti tornano nell'inventario,
/// i nuovi ne vengono prelevati. Tutto o niente.
pub async fn equip(
    State(s): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
    Json(e): Json<Equip>,
) -> ApiResult<Json<RosterUnit>> {
    ensure_idle(&s, user.id)?;
    let unit = owned_unit(&s, user.id, id).await?;
    let wanted = e
        .items
        .iter()
        .map(|k| parse_item(k))
        .collect::<ApiResult<Vec<_>>>()?;
    validate_loadout(unit.class(), &wanted)
        .map_err(|err| ApiError::bad_request(err.to_string()))?;

    let mut tx = s.db.begin().await?;
    for old in unit.items() {
        db::add_item(&mut tx, user.id, old, 1).await?;
    }
    for &new in &wanted {
        if !db::take_item(&mut tx, user.id, new).await? {
            return Err(ApiError::conflict(format!(
                "non possiedi {}",
                new.def().name
            )));
        }
    }
    sqlx::query("UPDATE roster SET equipment = ? WHERE id = ? AND account_id = ?")
        .bind(items_json(&wanted))
        .bind(id)
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(owned_unit(&s, user.id, id).await?))
}

#[derive(Deserialize)]
pub struct ChangeClass {
    class: String,
}

/// Cambia classe se sbloccata; gli oggetti non adatti tornano nell'inventario.
pub async fn change_class(
    State(s): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
    Json(c): Json<ChangeClass>,
) -> ApiResult<Json<RosterUnit>> {
    ensure_idle(&s, user.id)?;
    let unit = owned_unit(&s, user.id, id).await?;
    let class =
        Class::from_key(&c.class).ok_or_else(|| ApiError::bad_request("classe sconosciuta"))?;
    if !class.unlocked_for(unit.level as i32, &unit.mastered()) {
        return Err(ApiError::new(
            axum::http::StatusCode::FORBIDDEN,
            format!("{} richiede: {}", class.name(), class.requirement_text()),
        ));
    }
    let (keep, drop): (Vec<ItemId>, Vec<ItemId>) = unit
        .items()
        .into_iter()
        .partition(|i| i.def().usable_by(class));
    let mut tx = s.db.begin().await?;
    for i in drop {
        db::add_item(&mut tx, user.id, i, 1).await?;
    }
    sqlx::query("UPDATE roster SET class = ?, equipment = ? WHERE id = ? AND account_id = ?")
        .bind(class.key())
        .bind(items_json(&keep))
        .bind(id)
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(owned_unit(&s, user.id, id).await?))
}

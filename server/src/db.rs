use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{FromRow, SqliteConnection, SqlitePool};
use tactica_core::ability::AbilityId;
use tactica_core::class::Class;
use tactica_core::gear::ItemRef;
use tactica_core::progression;
use tactica_core::unit::Unit;
use tactica_core::{Dir, Pos, UnitSetup};

pub const MAX_ROSTER: i64 = 8;

pub async fn connect(url: &str) -> anyhow::Result<SqlitePool> {
    let opts = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5));
    // Un database in memoria esiste per connessione: una sola connessione condivisa.
    let max = if url.contains(":memory:") { 1 } else { 8 };
    let pool = SqlitePoolOptions::new()
        .max_connections(max)
        .connect_with(opts)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

/// Copia coerente del database (anche a server attivo) con rotazione.
pub async fn backup(db: &SqlitePool, dir: &str, keep: usize) -> anyhow::Result<String> {
    tokio::fs::create_dir_all(dir).await?;
    let path = format!("{dir}/tactica-{}.db", now());
    sqlx::query("VACUUM INTO ?").bind(&path).execute(db).await?;
    let mut files = Vec::new();
    let mut rd = tokio::fs::read_dir(dir).await?;
    while let Some(e) = rd.next_entry().await? {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with("tactica-") && name.ends_with(".db") {
            files.push(e.path());
        }
    }
    files.sort();
    let excess = files.len().saturating_sub(keep.max(1));
    for old in &files[..excess] {
        tokio::fs::remove_file(old).await?;
    }
    Ok(path)
}

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

#[derive(FromRow)]
struct RosterRow {
    id: i64,
    name: String,
    class: String,
    level: i64,
    xp: i64,
    equipment: String,
    bag: String,
    people: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct AbilityProgress {
    pub key: &'static str,
    pub name: &'static str,
    pub school: &'static str,
    pub rp: i64,
    pub master_rp: i64,
    pub mastered: bool,
    /// Insegnata da un oggetto equipaggiato (usabile ora, accumula RP).
    pub training: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct RosterUnit {
    pub id: i64,
    pub name: String,
    pub class: String,
    pub class_name: &'static str,
    pub level: i64,
    pub xp: i64,
    /// Chiavi degli oggetti equipaggiati.
    pub equipment: Vec<String>,
    /// Chiavi dei consumabili nella borsa di battaglia.
    pub bag: Vec<String>,
    pub people: &'static str,
    pub people_name: &'static str,
    /// Tratti del popolo in italiano.
    pub people_traits: &'static str,
    /// Movimento e salto con classe, popolo ed equipaggiamento.
    pub mov: i32,
    pub jump: i32,
    /// Abilità usabili in battaglia con la configurazione attuale.
    pub usable: Vec<&'static str>,
    /// Abilità con progressi (in addestramento o padroneggiate), di ogni scuola.
    pub abilities: Vec<AbilityProgress>,
    /// Classi a cui l'unità può passare.
    pub unlocked_classes: Vec<&'static str>,
}

impl RosterUnit {
    pub fn class(&self) -> Class {
        Class::from_key(&self.class).unwrap_or(Class::Vanguard)
    }

    pub fn items(&self) -> Vec<ItemRef> {
        self.equipment
            .iter()
            .filter_map(|k| ItemRef::from_key(k))
            .collect()
    }

    pub fn bag_items(&self) -> Vec<ItemRef> {
        self.bag
            .iter()
            .filter_map(|k| ItemRef::from_key(k))
            .collect()
    }

    pub fn mastered(&self) -> Vec<AbilityId> {
        self.abilities
            .iter()
            .filter(|a| a.mastered)
            .filter_map(|a| AbilityId::from_key(a.key))
            .collect()
    }

    pub fn to_setup(&self, team: u8) -> UnitSetup {
        UnitSetup {
            name: self.name.clone(),
            team,
            class: self.class(),
            level: self.level as i32,
            xp: self.xp as i32,
            equipment: self.items(),
            mastered: self.mastered(),
            bag: self.bag_items(),
            people: tactica_core::people::People::from_key(self.people).unwrap_or_default(),
        }
    }
}

pub fn parse_items(json: &str) -> Vec<ItemRef> {
    serde_json::from_str::<Vec<String>>(json)
        .unwrap_or_default()
        .iter()
        .filter_map(|k| ItemRef::from_key(k))
        .collect()
}

pub fn items_json(items: &[ItemRef]) -> String {
    serde_json::to_string(&items.iter().map(|i| i.key()).collect::<Vec<_>>()).expect("json")
}

fn people_of(key: &str) -> tactica_core::people::People {
    tactica_core::people::People::from_key(key).unwrap_or_default()
}

fn build_unit(row: RosterRow, progress: &[(i64, String, i64, bool)]) -> RosterUnit {
    let class = Class::from_key(&row.class).unwrap_or(Class::Vanguard);
    let equipment = parse_items(&row.equipment);
    let mine: Vec<(AbilityId, i64, bool)> = progress
        .iter()
        .filter(|p| p.0 == row.id)
        .filter_map(|p| AbilityId::from_key(&p.1).map(|a| (a, p.2, p.3)))
        .collect();
    let mastered: Vec<AbilityId> = mine.iter().filter(|m| m.2).map(|m| m.0).collect();
    let training = progression::training(class, &equipment, &mastered);

    let mut abilities = Vec::new();
    for a in AbilityId::ALL {
        let rec = mine.iter().find(|m| m.0 == a);
        let is_training = training.contains(&a);
        if rec.is_none() && !is_training {
            continue;
        }
        let d = a.def();
        abilities.push(AbilityProgress {
            key: d.key,
            name: d.name,
            school: a.school().map_or("", Class::key),
            rp: rec.map_or(0, |r| r.1),
            master_rp: d.master_rp as i64,
            mastered: rec.is_some_and(|r| r.2),
            training: is_training,
        });
    }

    let level = row.level as i32;
    let setup = UnitSetup {
        name: row.name.clone(),
        team: 0,
        class,
        level,
        xp: 0,
        equipment: equipment.clone(),
        mastered: mastered.clone(),
        bag: Vec::new(),
        people: people_of(&row.people),
    };
    let probe = Unit::new(0, &setup, Pos::new(0, 0), Dir::E);
    let usable = probe.abilities().iter().map(|a| a.key()).collect();
    let people = people_of(&row.people);
    RosterUnit {
        id: row.id,
        name: row.name,
        class: class.key().to_string(),
        class_name: class.name(),
        level: row.level,
        xp: row.xp,
        equipment: equipment.iter().map(|i| i.key().to_string()).collect(),
        bag: parse_items(&row.bag).iter().map(|i| i.key()).collect(),
        people: people.key(),
        people_name: people.name(),
        people_traits: people.traits(),
        mov: probe.move_points(),
        jump: probe.jump_points(),
        usable,
        abilities,
        unlocked_classes: Class::ALL
            .into_iter()
            .filter(|c| c.unlocked_for(level, &mastered))
            .map(Class::key)
            .collect(),
    }
}

pub async fn roster(db: &SqlitePool, account: i64) -> sqlx::Result<Vec<RosterUnit>> {
    let rows: Vec<RosterRow> = sqlx::query_as(
        "SELECT id, name, class, level, xp, equipment, bag, people FROM roster WHERE account_id = ? ORDER BY id",
    )
    .bind(account)
    .fetch_all(db)
    .await?;
    let progress: Vec<(i64, String, i64, bool)> = sqlx::query_as(
        "SELECT ra.roster_id, ra.ability, ra.rp, ra.mastered
         FROM roster_abilities ra JOIN roster r ON r.id = ra.roster_id
         WHERE r.account_id = ?",
    )
    .bind(account)
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|r| build_unit(r, &progress)).collect())
}

pub async fn roster_unit(
    db: &SqlitePool,
    account: i64,
    id: i64,
) -> sqlx::Result<Option<RosterUnit>> {
    Ok(roster(db, account).await?.into_iter().find(|u| u.id == id))
}

/// Unità del roster richieste, nell'ordine dato; `None` se qualcuna non appartiene all'account.
pub async fn roster_units(
    db: &SqlitePool,
    account: i64,
    ids: &[i64],
) -> sqlx::Result<Option<Vec<RosterUnit>>> {
    let all = roster(db, account).await?;
    Ok(ids
        .iter()
        .map(|id| all.iter().find(|u| u.id == *id).cloned())
        .collect())
}

// ---------------------------------------------------------------- inventario

pub async fn add_item(
    conn: &mut SqliteConnection,
    account: i64,
    item: ItemRef,
    n: i64,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO inventory (account_id, item, qty) VALUES (?, ?, ?)
         ON CONFLICT(account_id, item) DO UPDATE SET qty = qty + excluded.qty",
    )
    .bind(account)
    .bind(item.key())
    .bind(n)
    .execute(conn)
    .await?;
    Ok(())
}

/// Toglie un esemplare dall'inventario; `false` se non posseduto.
pub async fn take_item(
    conn: &mut SqliteConnection,
    account: i64,
    item: ItemRef,
) -> sqlx::Result<bool> {
    let n = sqlx::query(
        "UPDATE inventory SET qty = qty - 1 WHERE account_id = ? AND item = ? AND qty > 0",
    )
    .bind(account)
    .bind(item.key())
    .execute(conn)
    .await?
    .rows_affected();
    Ok(n == 1)
}

pub async fn inventory(db: &SqlitePool, account: i64) -> sqlx::Result<Vec<(String, i64)>> {
    sqlx::query_as("SELECT item, qty FROM inventory WHERE account_id = ? AND qty > 0 ORDER BY item")
        .bind(account)
        .fetch_all(db)
        .await
}

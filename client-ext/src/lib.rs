//! GDExtension: espone il motore di regole `tactica-core` a Godot.
//!
//! - Online: `from_snapshot` ricostruisce una battaglia di sola consultazione
//!   dallo snapshot del server (caselle raggiungibili, bersagli, anteprime, IA).
//! - Offline: `new_local` crea una battaglia completa per la schermaglia locale.
//!
//! Lo scambio di dati strutturati avviene in JSON (stringhe), così GDScript non
//! dipende dai nomi interni dei tipi Rust.

// Il derive di gdext genera inizializzazioni `base: base`.
#![allow(clippy::redundant_field_names)]

use godot::prelude::*;
use serde_json::{Value, json};
use tactica_core::ability::AbilityId;
use tactica_core::ai;
use tactica_core::presets;
use tactica_core::text::describe_event;
use tactica_core::{Action, Battle, BattleSetup, Dir, Event, Pos, Snapshot, UnitSetup};

struct TacticaExtension;

#[gdextension]
unsafe impl ExtensionLibrary for TacticaExtension {}

#[derive(GodotClass)]
#[class(init, base = RefCounted)]
pub struct TacticaEngine {
    battle: Option<Battle>,
    local: bool,
    base: Base<RefCounted>,
}

fn gs(s: impl AsRef<str>) -> GString {
    GString::from(s.as_ref())
}

fn pos(v: Vector2i) -> Pos {
    Pos::new(v.x, v.y)
}

fn vec2i(p: Pos) -> Vector2i {
    Vector2i::new(p.x, p.y)
}

fn ability(key: &GString) -> Option<AbilityId> {
    AbilityId::from_key(&key.to_string())
}

fn action_json(a: Action) -> GString {
    gs(serde_json::to_string(&a).expect("json"))
}

#[godot_api]
impl TacticaEngine {
    /// Battaglia locale completa (schermaglia offline). `units_json`: lista di UnitSetup.
    #[func]
    fn new_local(seed: i64, units_json: GString, statutes: i64) -> Option<Gd<Self>> {
        let units: Vec<UnitSetup> = match serde_json::from_str(&units_json.to_string()) {
            Ok(u) => u,
            Err(e) => {
                godot_error!("squadre non valide: {e}");
                return None;
            }
        };
        let setup = BattleSetup {
            seed: seed as u64,
            map: None,
            units,
            statutes: statutes.max(0) as usize,
            objective: Default::default(),
            // Schermaglia locale: i nemici sconfitti lasciano oggetti (non salvati).
            loot: tactica_core::LootRules {
                chance: [0, 40],
                boost: [0, 1],
                champion: None,
            },
        };
        let (battle, _) = Battle::new(&setup);
        Some(Gd::from_init_fn(|base| Self {
            battle: Some(battle),
            local: true,
            base,
        }))
    }

    /// Battaglia di sola consultazione dallo snapshot JSON del server.
    #[func]
    fn from_snapshot(json: GString) -> Option<Gd<Self>> {
        let snap: Snapshot = serde_json::from_str(&json.to_string())
            .map_err(|e| {
                godot_error!("snapshot non valido: {e}");
            })
            .ok()?;
        let battle = Battle::from_snapshot(snap);
        Some(Gd::from_init_fn(|base| Self {
            battle: Some(battle),
            local: false,
            base,
        }))
    }

    /// Squadra dimostrativa (4 classi base equipaggiate) in JSON.
    #[func]
    fn demo_party_json(team: i64, level: i64) -> GString {
        let names = if team == 0 {
            ["Alda", "Brenno", "Cora", "Dario"]
        } else {
            ["Ezio", "Fosca", "Gualtiero", "Ilaria"]
        };
        gs(serde_json::to_string(&if team == 0 {
            // La squadra blu usa l'equipaggiamento con gli status, per mostrarli in azione.
            presets::status_party(0, level as i32, names)
        } else {
            presets::demo_party(1, level as i32, names)
        })
        .expect("json"))
    }

    #[func]
    fn is_local(&self) -> bool {
        self.local
    }

    #[func]
    fn snapshot_json(&self) -> GString {
        match &self.battle {
            Some(b) => gs(serde_json::to_string(&b.snapshot()).expect("json")),
            None => GString::new(),
        }
    }

    /// Applica un'azione (solo battaglie locali): `{"events":[...]}` o `{"error":"..."}`.
    #[func]
    fn apply_json(&mut self, action_json: GString) -> GString {
        let out = match (&mut self.battle, self.local) {
            (Some(b), true) => match serde_json::from_str::<Action>(&action_json.to_string()) {
                Ok(a) => match b.apply(a) {
                    Ok(ev) => json!({ "events": ev }),
                    Err(e) => json!({ "error": e.to_string() }),
                },
                Err(e) => json!({ "error": format!("azione non valida: {e}") }),
            },
            _ => json!({ "error": "battaglia non locale" }),
        };
        gs(out.to_string())
    }

    #[func]
    fn reachable(&self, unit: i64) -> Array<Vector2i> {
        let mut out = Array::new();
        if let Some(b) = &self.battle
            && (0..b.units.len() as i64).contains(&unit)
        {
            for p in b.reachable(unit as usize) {
                out.push(vec2i(p));
            }
        }
        out
    }

    /// Caselle bersagliabili con l'abilità dalla posizione attuale dell'unità.
    #[func]
    fn targets(&self, unit: i64, ability_key: GString) -> Array<Vector2i> {
        let mut out = Array::new();
        let (Some(b), Some(a)) = (&self.battle, ability(&ability_key)) else {
            return out;
        };
        let Some(u) = b.units.get(unit as usize) else {
            return out;
        };
        for p in b.map.positions() {
            if b.check_target(unit as usize, u.pos, a, p).is_ok() {
                out.push(vec2i(p));
            }
        }
        out
    }

    /// Anteprima: effetti su ogni unità coinvolta e statuti violati.
    #[func]
    fn preview_json(&self, unit: i64, ability_key: GString, target: Vector2i) -> GString {
        match ability(&ability_key) {
            Some(a) => self.preview_cast(unit, a.into(), target),
            None => gs("{}"),
        }
    }

    /// Consumabile in `slot` della borsa dell'unità (se c'è).
    fn bag_cast(&self, unit: i64, slot: i64) -> Option<tactica_core::consumable::Cast> {
        let b = self.battle.as_ref()?;
        b.item_cast(unit as usize, slot as u8).map(|(_, cast)| cast)
    }

    /// Caselle bersagliabili con il consumabile in `slot` della borsa.
    #[func]
    fn item_targets(&self, unit: i64, slot: i64) -> Array<Vector2i> {
        let mut out = Array::new();
        let (Some(b), Some(cast)) = (&self.battle, self.bag_cast(unit, slot)) else {
            return out;
        };
        let u = &b.units[unit as usize];
        for p in b.map.positions() {
            if b.check_cast(unit as usize, u.pos, cast, p, true).is_ok() {
                out.push(vec2i(p));
            }
        }
        out
    }

    /// Anteprima del consumabile in `slot` della borsa sul bersaglio.
    #[func]
    fn item_preview_json(&self, unit: i64, slot: i64, target: Vector2i) -> GString {
        match self.bag_cast(unit, slot) {
            Some(cast) => self.preview_cast(unit, cast, target),
            None => gs("{}"),
        }
    }

    fn preview_cast(
        &self,
        unit: i64,
        cast: tactica_core::consumable::Cast,
        target: Vector2i,
    ) -> GString {
        let a = cast.ability;
        let Some(b) = &self.battle else {
            return gs("{}");
        };
        let Some(u) = b.units.get(unit as usize) else {
            return gs("{}");
        };
        let t = pos(target);
        let outcomes: Vec<Value> = b
            .outcomes_cast(unit as usize, u.pos, cast, t)
            .into_iter()
            .map(|o| {
                let tu = &b.units[o.target];
                json!({
                    "unit": o.target,
                    "name": tu.name,
                    "hit": o.hit_pct,
                    "amount": o.amount,
                    "heal": o.heal,
                    "lethal": !o.heal && o.amount > 0 && o.amount >= tu.hp,
                    "ally": tu.team == u.team,
                    "status": o.status.map(|e| e.status.name()),
                    "status_chance": o.status.map_or(0, |e| e.chance * o.hit_pct / 100),
                    "cleanse": o.cleanse,
                })
            })
            .collect();
        let violations: Vec<String> = b.violations(u.pos, a, t).iter().map(|s| s.name()).collect();
        let area: Vec<Value> = b
            .map
            .positions()
            .filter(|p| p.dist(t) <= a.def().aoe)
            .map(|p| json!([p.x, p.y]))
            .collect();
        gs(json!({ "outcomes": outcomes, "violations": violations, "area": area }).to_string())
    }

    /// Abilità dell'unità: chiave, nome, costo, utilizzabile ora.
    #[func]
    fn abilities_json(&self, unit: i64) -> GString {
        let Some(u) = self
            .battle
            .as_ref()
            .and_then(|b| b.units.get(unit as usize))
        else {
            return gs("[]");
        };
        let list: Vec<Value> = u
            .abilities()
            .into_iter()
            .map(|a| {
                let d = a.def();
                json!({
                    "key": d.key,
                    "name": d.name,
                    "mp_cost": d.mp_cost,
                    "usable": u.mp >= d.mp_cost,
                    "range": [d.range_min, d.range_max],
                    "aoe": d.aoe,
                    "element": d.element.map(|e| e.name()),
                    "status": d.effect.map(|e| e.status.name()),
                })
            })
            .collect();
        gs(Value::Array(list).to_string())
    }

    /// Prossima azione suggerita dall'IA per l'unità attiva (JSON) o stringa vuota.
    #[func]
    fn ai_next_json(&self) -> GString {
        match self
            .battle
            .as_ref()
            .and_then(|b| ai::plan_turn(b).into_iter().next())
        {
            Some(a) => action_json(a),
            None => GString::new(),
        }
    }

    /// Descrizioni in italiano di una lista di eventi JSON.
    #[func]
    fn describe_events(&self, events_json: GString) -> Array<GString> {
        let mut out = Array::new();
        let Some(b) = &self.battle else { return out };
        let Ok(events) = serde_json::from_str::<Vec<Event>>(&events_json.to_string()) else {
            return out;
        };
        let name = |id: u32| {
            b.units
                .get(id as usize)
                .map_or("?".to_string(), |u| u.name.clone())
        };
        for e in &events {
            if let Some(line) = describe_event(e, &name) {
                out.push(&gs(line.trim_start()));
            }
        }
        out
    }

    /// Aggiorna la battaglia di consultazione dal testo grezzo di un messaggio
    /// del server che contiene `snapshot` (evita conversioni numeriche di GDScript).
    #[func]
    fn set_snapshot_from_message(&mut self, raw: GString) -> bool {
        let Ok(v) = serde_json::from_str::<Value>(&raw.to_string()) else {
            return false;
        };
        let Some(snap) = v.get("snapshot").cloned() else {
            return false;
        };
        match serde_json::from_value::<Snapshot>(snap) {
            Ok(s) => {
                self.battle = Some(Battle::from_snapshot(s));
                self.local = false;
                true
            }
            Err(e) => {
                godot_error!("snapshot non valido: {e}");
                false
            }
        }
    }

    /// Descrive il campo `events` di un messaggio JSON (server o `apply_json`).
    #[func]
    fn describe_message_events(&self, raw: GString) -> Array<GString> {
        let events = serde_json::from_str::<Value>(&raw.to_string())
            .ok()
            .and_then(|v| v.get("events").cloned())
            .unwrap_or(Value::Array(vec![]));
        self.describe_events(gs(events.to_string()))
    }

    /// Vista delle unità per l'interfaccia (nomi italiani, valori interi).
    #[func]
    fn units_view_json(&self) -> GString {
        let Some(b) = &self.battle else {
            return gs("[]");
        };
        let list: Vec<Value> = b
            .units
            .iter()
            .enumerate()
            .map(|(i, u)| {
                json!({
                    "index": i,
                    "name": u.name,
                    "team": u.team,
                    "class": u.class.key(),
                    "class_name": u.class.name(),
                    "level": u.level,
                    "hp": u.hp,
                    "max_hp": u.stats.max_hp,
                    "mp": u.mp,
                    "max_mp": u.stats.max_mp,
                    "x": u.pos.x,
                    "y": u.pos.y,
                    "facing": format!("{:?}", u.facing),
                    "active": u.is_active(),
                    "condition": format!("{:?}", u.condition),
                    "suspended": u.suspended,
                    "penalties": u.penalties,
                    "statuses": u.statuses.iter().map(|(s, t)| json!({
                        "key": s.key(), "name": s.name(), "turns": t, "negative": s.is_negative(),
                    })).collect::<Vec<_>>(),
                    "bag": u.bag.iter().map(|i| i.map(|i| json!({
                        "key": i.key(), "name": i.name(),
                    }))).collect::<Vec<_>>(),
                    "people": u.people.key(),
                    "people_name": u.people.name(),
                    // Movimento e salto effettivi (status, popolo, classe, armatura).
                    "stats": {
                        "atk": u.stats.atk, "def": u.stats.def, "mag": u.stats.mag,
                        "res": u.stats.res, "spd": u.stats.spd, "mov": u.move_points(),
                        "jump": u.jump_points(), "evade": u.stats.evade,
                        "base_mov": u.stats.mov, "base_jump": u.stats.jump,
                    },
                })
            })
            .collect();
        gs(Value::Array(list).to_string())
    }

    #[func]
    fn statute_names(&self) -> Array<GString> {
        let mut out = Array::new();
        if let Some(b) = &self.battle {
            for s in &b.statutes {
                out.push(&gs(s.name()));
            }
        }
        out
    }

    /// Casella: terreno, altezza, effetti del terreno e unità presente.
    #[func]
    fn tile_info_json(&self, target: Vector2i) -> GString {
        let Some(b) = &self.battle else {
            return gs("{}");
        };
        let p = pos(target);
        let Some(t) = b.map.tile(p) else {
            return gs("{}");
        };
        let unit = b.unit_at(p).map(|i| b.units[i].name.clone());
        let loot: Vec<String> = b.loot_at(p).map(|d| d.item.key()).collect();
        gs(json!({
            "terrain": t.terrain.name(),
            "height": t.height,
            "effects": t.terrain.effects(),
            "unit": unit,
            "loot": loot,
        })
        .to_string())
    }

    /// Nome del bioma della mappa.
    #[func]
    fn biome_name(&self) -> GString {
        self.battle
            .as_ref()
            .map_or(GString::new(), |b| gs(b.map.biome.name()))
    }

    /// Versione di compatibilità con il server (regole.protocollo).
    #[func]
    fn compat_version() -> GString {
        gs(tactica_core::compat_version())
    }

    /// Popoli giocabili: `[{"key","name","traits"}]`.
    #[func]
    fn peoples_json() -> GString {
        use tactica_core::people::People;
        let v: Vec<Value> = People::ALL
            .iter()
            .map(|p| json!({"key": p.key(), "name": p.name(), "traits": p.traits()}))
            .collect();
        gs(Value::Array(v).to_string())
    }

    /// Scheda di un oggetto qualsiasi (del catalogo o generato) dalla sua chiave,
    /// nello stesso formato di `GET /api/items`; `{}` se la chiave non è valida.
    #[func]
    fn item_info_json(key: GString) -> GString {
        use tactica_core::class::Class;
        use tactica_core::gear::ItemRef;
        let Some(i) = ItemRef::from_key(&key.to_string()) else {
            return gs("{}");
        };
        let classes: Vec<&str> = if i.classes().is_empty() {
            Class::ALL.iter().map(|c| c.key()).collect()
        } else {
            i.classes().iter().map(|c| c.key()).collect()
        };
        gs(json!({
            "key": i.key(),
            "name": i.name(),
            "slot": i.slot().name(),
            "slot_key": i.slot().key(),
            "classes": classes,
            "bonus": serde_json::to_value(i.bonus()).unwrap_or_default(),
            "teaches": i.teaches().map(AbilityId::key),
            "price": i.price(),
            "in_shop": matches!(i, ItemRef::Fixed(f) if f.def().in_shop),
            "rarity": i.rarity().key(),
            "rarity_name": i.rarity().name(),
            "effects": i.effects().into_iter().map(|e| e.describe()).collect::<Vec<_>>(),
            "icon": i.icon(),
            "lore": i.lore(),
        })
        .to_string())
    }

    /// Effetto visivo di un'abilità (nome della variante, es. "Flame"):
    /// `{"fx": ..., "projectile": bool, "aoe": n}`.
    #[func]
    fn ability_fx(variant: GString) -> GString {
        use tactica_core::ability::{Element, Kind};
        let Ok(a) = serde_json::from_value::<AbilityId>(Value::String(variant.to_string())) else {
            return gs("{}");
        };
        let d = a.def();
        let fx = match (d.kind, d.element) {
            (Kind::Heal, _) => "heal",
            (Kind::Support, _) if d.cleanse => "heal",
            (Kind::Support, _) => match d.effect {
                Some(e) if e.status.is_negative() => "debuff",
                _ => "buff",
            },
            (_, Some(Element::Fire)) => "fire",
            (_, Some(Element::Ice)) => "ice",
            (_, Some(Element::Thunder)) => "thunder",
            (_, Some(Element::Holy)) => "holy",
            (Kind::Physical, None) if d.drain => "drain",
            (Kind::Physical, None) if d.ranged => "impact",
            (Kind::Physical, None) => "slash",
            (Kind::Magic, None) => "debuff",
        };
        // Suono: arma (taglio, affondo, contundente, freccia, lancio), elemento,
        // cura o rianimazione; per i supporti un incantesimo generico (lo status
        // ha poi il suo suono quando viene applicato).
        let sound = match (d.kind, d.element) {
            _ if d.revive => "revive",
            _ if d.restore_mp => "mp",
            (Kind::Heal, _) => "heal",
            (Kind::Support, _) if d.cleanse => "cleanse",
            (Kind::Support, _) => "magic",
            (Kind::Physical, _) if d.key == "lob" => "throw",
            (Kind::Physical, _) if d.ranged => "arrow",
            (Kind::Physical, _) if d.drain => "drain",
            (Kind::Physical, Some(Element::Fire)) => "fire",
            (Kind::Physical, Some(Element::Ice)) => "ice",
            (Kind::Physical, Some(Element::Thunder)) => "thunder",
            (Kind::Physical, Some(Element::Holy)) => "blunt",
            (Kind::Physical, None)
                if matches!(
                    d.key,
                    "charge" | "leap" | "dive_strike" | "dragon_lance" | "backstab"
                ) =>
            {
                "pierce"
            }
            (Kind::Physical, None) if d.power >= 160 => "slash_heavy",
            (Kind::Physical, None) => "slash",
            (_, Some(Element::Fire)) => "fire",
            (_, Some(Element::Ice)) => "ice",
            (_, Some(Element::Thunder)) => "thunder",
            (_, Some(Element::Holy)) => "holy",
            (Kind::Magic, None) => "magic",
        };
        gs(json!({ "fx": fx, "projectile": d.ranged, "aoe": d.aoe, "sound": sound }).to_string())
    }

    #[func]
    fn state_hash(&self) -> i64 {
        self.battle.as_ref().map_or(0, |b| b.state_hash() as i64)
    }

    #[func]
    fn make_move(x: i64, y: i64) -> GString {
        action_json(Action::Move(Pos::new(x as i32, y as i32)))
    }

    #[func]
    fn make_act(ability_key: GString, x: i64, y: i64) -> GString {
        match ability(&ability_key) {
            Some(a) => action_json(Action::Act {
                ability: a,
                target: Pos::new(x as i32, y as i32),
            }),
            None => GString::new(),
        }
    }

    /// Azione: usa il consumabile in `slot` della borsa sulla casella (x, y).
    #[func]
    fn make_use_item(slot: i64, x: i64, y: i64) -> GString {
        action_json(Action::UseItem {
            slot: slot as u8,
            target: Pos::new(x as i32, y as i32),
        })
    }

    /// `dir`: "N", "E", "S" o "W".
    #[func]
    fn make_end_turn(dir: GString) -> GString {
        let d = match dir.to_string().as_str() {
            "N" => Dir::N,
            "S" => Dir::S,
            "W" => Dir::W,
            _ => Dir::E,
        };
        action_json(Action::EndTurn(d))
    }

    #[func]
    fn make_concede(team: i64) -> GString {
        action_json(Action::Concede(team as u8))
    }
}

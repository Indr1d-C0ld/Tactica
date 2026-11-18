//! Traguardi: un catalogo ampio che copre tutto il gioco (battaglia, classi,
//! popoli, crescita, abilità e status, bottino, economia, bottega, campagna,
//! PvP e stagioni, clan, guerre e tornei, statuti, ambienti, fedeltà e segreti).
//!
//! Ogni traguardo è una soglia su un **contatore** dell'account (tabella
//! `counters`): `bump` lo incrementa, `peak` lo porta al massimo visto. I
//! contatori si aggiornano nei punti del gioco in cui succedono le cose (fine
//! partita, negozio, bottega, reclute, clan, tornei, stagioni); alcuni («vivi»)
//! si ricalcolano dal database a ogni controllo. `check` sblocca i traguardi
//! raggiunti, dà le monete del grado e, per il platino, il titolo.
//!
//! I fatti di una partita (colpi, status, rianimazioni, bottino...) si
//! ricavano rigiocando le azioni salvate: il motore è deterministico.

#![allow(clippy::type_complexity)]

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use sqlx::SqliteConnection;
use tactica_core::class::Class;
use tactica_core::people::People;
use tactica_core::status::Status;
use tactica_core::unit::Condition;
use tactica_core::{Action, Battle, BattleSetup, Event};

use crate::db::now;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Bronze,
    Silver,
    Gold,
    Platinum,
}

impl Tier {
    pub fn coins(self) -> i64 {
        match self {
            Tier::Bronze => 25,
            Tier::Silver => 75,
            Tier::Gold => 200,
            Tier::Platinum => 500,
        }
    }
}

/// Un traguardo del catalogo.
#[derive(Clone, Debug, Serialize)]
pub struct Def {
    pub id: String,
    pub name: String,
    pub desc: String,
    pub category: &'static str,
    pub tier: Tier,
    /// Nascosto finché non si sblocca.
    pub secret: bool,
    pub counter: String,
    pub target: i64,
}

/// Le categorie, nell'ordine in cui si mostrano.
pub const CATEGORIES: [(&str, &str); 15] = [
    ("battaglia", "Battaglia"),
    ("classi", "Classi"),
    ("popoli", "Popoli"),
    ("crescita", "Crescita"),
    ("arti", "Abilità e status"),
    ("bottino", "Bottino"),
    ("economia", "Economia"),
    ("bottega", "Bottega"),
    ("campagna", "Campagna"),
    ("pvp", "Classificata e stagioni"),
    ("clan", "Clan, guerre e tornei"),
    ("statuti", "Statuti"),
    ("ambienti", "Ambienti e viaggi"),
    ("fedelta", "Fedeltà"),
    ("segreti", "Segreti"),
];

fn biome_it(key: &str) -> &'static str {
    match key {
        "meadow" => "Prateria",
        "desert" => "Deserto",
        "volcano" => "Vulcano",
        "marsh" => "Palude",
        _ => "Altopiano",
    }
}

const BIOMES: [&str; 5] = ["meadow", "desert", "volcano", "marsh", "highlands"];

/// Il catalogo completo (costruito una volta sola).
pub fn catalog() -> &'static [Def] {
    use std::sync::OnceLock;
    static CAT: OnceLock<Vec<Def>> = OnceLock::new();
    CAT.get_or_init(build)
}

fn build() -> Vec<Def> {
    use Tier::*;
    let mut v: Vec<Def> = Vec::new();
    let mut add = |id: &str,
                   name: &str,
                   desc: &str,
                   cat: &'static str,
                   tier: Tier,
                   counter: &str,
                   target: i64| {
        v.push(Def {
            id: id.into(),
            name: name.into(),
            desc: desc.into(),
            category: cat,
            tier,
            secret: cat == "segreti",
            counter: counter.into(),
            target,
        })
    };
    // Una scala di soglie sullo stesso contatore.
    macro_rules! ladder {
        ($cat:expr, $counter:expr, $( ($id:expr, $name:expr, $desc:expr, $tier:expr, $n:expr) ),+ $(,)?) => {
            $( add($id, $name, $desc, $cat, $tier, $counter, $n); )+
        };
    }

    // ------------------------------------------------------------ battaglia
    ladder!(
        "battaglia",
        "matches",
        (
            "partite_1",
            "Primo sangue",
            "Gioca la tua prima partita.",
            Bronze,
            1
        ),
        ("partite_10", "Veterano", "Gioca 10 partite.", Bronze, 10),
        (
            "partite_50",
            "Una vita sul campo",
            "Gioca 50 partite.",
            Silver,
            50
        ),
        (
            "partite_200",
            "Il campo è casa",
            "Gioca 200 partite.",
            Gold,
            200
        ),
        (
            "partite_1000",
            "Leggenda vivente",
            "Gioca 1000 partite.",
            Platinum,
            1000
        )
    );
    ladder!(
        "battaglia",
        "wins",
        (
            "vittorie_1",
            "La prima vittoria",
            "Vinci una partita.",
            Bronze,
            1
        ),
        (
            "vittorie_10",
            "Abituato a vincere",
            "Vinci 10 partite.",
            Bronze,
            10
        ),
        (
            "vittorie_50",
            "Condottiero",
            "Vinci 50 partite.",
            Silver,
            50
        ),
        ("vittorie_250", "Stratega", "Vinci 250 partite.", Gold, 250),
        (
            "vittorie_1000",
            "Imbattibile",
            "Vinci 1000 partite.",
            Platinum,
            1000
        )
    );
    ladder!(
        "battaglia",
        "kills",
        ("ko_10", "Mano ferma", "Metti KO 10 nemici.", Bronze, 10),
        (
            "ko_100",
            "Cento abbattuti",
            "Metti KO 100 nemici.",
            Silver,
            100
        ),
        ("ko_500", "Flagello", "Metti KO 500 nemici.", Gold, 500),
        (
            "ko_2500",
            "Mietitore",
            "Metti KO 2500 nemici.",
            Platinum,
            2500
        )
    );
    ladder!(
        "battaglia",
        "damage",
        (
            "danni_10k",
            "Colpi pesanti",
            "Infliggi 10.000 danni in tutto.",
            Bronze,
            10_000
        ),
        (
            "danni_100k",
            "Distruttore",
            "Infliggi 100.000 danni in tutto.",
            Silver,
            100_000
        ),
        (
            "danni_1m",
            "Cataclisma",
            "Infliggi un milione di danni in tutto.",
            Gold,
            1_000_000
        )
    );
    ladder!(
        "battaglia",
        "healing",
        (
            "cure_5k",
            "Mani gentili",
            "Cura 5.000 HP in tutto.",
            Bronze,
            5_000
        ),
        (
            "cure_50k",
            "Sorgente di vita",
            "Cura 50.000 HP in tutto.",
            Silver,
            50_000
        ),
        (
            "cure_500k",
            "Miracolo continuo",
            "Cura 500.000 HP in tutto.",
            Gold,
            500_000
        )
    );
    ladder!(
        "battaglia",
        "max_hit",
        (
            "colpo_150",
            "Botta secca",
            "Infliggi 150 danni con un solo colpo.",
            Bronze,
            150
        ),
        (
            "colpo_300",
            "Spaccascudi",
            "Infliggi 300 danni con un solo colpo.",
            Silver,
            300
        ),
        (
            "colpo_600",
            "Colpo del secolo",
            "Infliggi 600 danni con un solo colpo.",
            Gold,
            600
        )
    );
    ladder!(
        "battaglia",
        "flawless",
        (
            "perfetta_1",
            "Nessun caduto",
            "Vinci senza che nessuna tua unità vada KO.",
            Silver,
            1
        ),
        (
            "perfetta_10",
            "Intoccabili",
            "Vinci 10 partite senza perdere unità.",
            Gold,
            10
        ),
        (
            "perfetta_50",
            "Disciplina di ferro",
            "Vinci 50 partite senza perdere unità.",
            Platinum,
            50
        )
    );
    ladder!(
        "battaglia",
        "clutch",
        (
            "ultimo_1",
            "L'ultimo in piedi",
            "Vinci con una sola unità sopravvissuta.",
            Silver,
            1
        ),
        (
            "ultimo_10",
            "Sempre sul filo",
            "Vinci 10 partite con un solo sopravvissuto.",
            Gold,
            10
        )
    );
    ladder!(
        "battaglia",
        "quick_wins",
        (
            "lampo_1",
            "Guerra lampo",
            "Vinci in 30 turni o meno.",
            Silver,
            1
        ),
        (
            "lampo_10",
            "Blitz",
            "Vinci 10 partite in 30 turni o meno.",
            Gold,
            10
        )
    );
    add(
        "lunga_1",
        "Battaglia infinita",
        "Gioca una partita di almeno 200 turni.",
        "battaglia",
        Bronze,
        "long_matches",
        1,
    );
    ladder!(
        "battaglia",
        "draws",
        (
            "pari_1",
            "Nessuno vince",
            "Pareggia una partita.",
            Bronze,
            1
        ),
        (
            "pari_10",
            "Equilibrio perfetto",
            "Pareggia 10 partite.",
            Silver,
            10
        )
    );
    ladder!(
        "battaglia",
        "losses",
        (
            "sconfitte_10",
            "Si impara cadendo",
            "Perdi 10 partite: succede ai migliori.",
            Bronze,
            10
        ),
        (
            "sconfitte_100",
            "Testa dura",
            "Perdi 100 partite e continua a giocare.",
            Silver,
            100
        )
    );
    ladder!(
        "battaglia",
        "best_kills_unit",
        (
            "strage_5",
            "Furia",
            "Una sola unità mette KO 5 nemici in una partita.",
            Silver,
            5
        ),
        (
            "strage_8",
            "Esercito di uno",
            "Una sola unità mette KO 8 nemici in una partita.",
            Gold,
            8
        )
    );
    ladder!(
        "battaglia",
        "multi_kill",
        (
            "doppio_1",
            "Due in un colpo",
            "Metti KO due nemici con una sola azione.",
            Silver,
            2
        ),
        (
            "triplo_1",
            "Tris",
            "Metti KO tre nemici con una sola azione.",
            Gold,
            3
        )
    );

    // ------------------------------------------------------------ classi
    for c in Class::ALL {
        let k = c.key();
        add(
            &format!("vinci_{k}"),
            &format!("Vittoria: {}", c.name()),
            &format!("Vinci una partita schierando un'unità {}.", c.name()),
            "classi",
            Bronze,
            &format!("class_win:{k}"),
            1,
        );
        add(
            &format!("veterano_{k}"),
            &format!("Veterano: {}", c.name()),
            &format!("Vinci 25 partite schierando un'unità {}.", c.name()),
            "classi",
            Silver,
            &format!("class_win:{k}"),
            25,
        );
    }
    for c in Class::ALL.into_iter().filter(|c| !c.is_base()) {
        add(
            &format!("diventa_{}", c.key()),
            &format!("Nuova via: {}", c.name()),
            &format!("Porta un'unità alla classe {}.", c.name()),
            "classi",
            Silver,
            &format!("class_reached:{}", c.key()),
            1,
        );
    }
    add(
        "classi_tutte",
        "Maestro di tutte le arti",
        "Porta le tue unità a tutte le 12 classi.",
        "classi",
        Platinum,
        "classes_reached",
        12,
    );
    add(
        "classi_meta",
        "A metà strada",
        "Porta le tue unità a 6 classi diverse.",
        "classi",
        Gold,
        "classes_reached",
        6,
    );

    // ------------------------------------------------------------ popoli
    for p in People::ALL {
        let k = p.key();
        add(
            &format!("recluta_{k}"),
            &format!("Alleanza: {}", p.name()),
            &format!("Recluta un'unità del popolo {}.", p.name()),
            "popoli",
            Bronze,
            &format!("people:{k}"),
            1,
        );
        add(
            &format!("puri_{k}"),
            &format!("Sangue {}", p.name()),
            &format!(
                "Vinci con una squadra di soli {} (almeno 3 unità).",
                p.name()
            ),
            "popoli",
            Silver,
            &format!("pure_win:{k}"),
            1,
        );
    }
    add(
        "popoli_tutti",
        "Le cinque genti",
        "Recluta unità di tutti e cinque i popoli.",
        "popoli",
        Gold,
        "peoples_recruited",
        5,
    );
    add(
        "popoli_misti",
        "Uniti nella diversità",
        "Vinci con quattro unità di quattro popoli diversi.",
        "popoli",
        Gold,
        "mixed_win",
        1,
    );

    // ------------------------------------------------------------ crescita
    ladder!(
        "crescita",
        "max_level",
        (
            "livello_5",
            "Recluta promettente",
            "Porta un'unità al livello 5.",
            Bronze,
            5
        ),
        (
            "livello_10",
            "Soldato esperto",
            "Porta un'unità al livello 10.",
            Bronze,
            10
        ),
        (
            "livello_20",
            "Ufficiale",
            "Porta un'unità al livello 20.",
            Silver,
            20
        ),
        (
            "livello_35",
            "Campione",
            "Porta un'unità al livello 35.",
            Gold,
            35
        ),
        (
            "livello_50",
            "Apice",
            "Porta un'unità al livello massimo, il 50.",
            Platinum,
            50
        )
    );
    ladder!(
        "crescita",
        "level_ups",
        (
            "crescite_10",
            "Si cresce",
            "Guadagna 10 livelli in tutto.",
            Bronze,
            10
        ),
        (
            "crescite_100",
            "Scuola di guerra",
            "Guadagna 100 livelli in tutto.",
            Silver,
            100
        ),
        (
            "crescite_400",
            "Accademia",
            "Guadagna 400 livelli in tutto.",
            Gold,
            400
        )
    );
    ladder!(
        "crescita",
        "mastered",
        (
            "padrone_1",
            "Risonanza",
            "Padroneggia un'abilità.",
            Bronze,
            1
        ),
        (
            "padrone_10",
            "Dieci arti",
            "Padroneggia 10 abilità.",
            Silver,
            10
        ),
        (
            "padrone_40",
            "Enciclopedia vivente",
            "Padroneggia 40 abilità.",
            Gold,
            40
        ),
        (
            "padrone_80",
            "Ogni arte conosciuta",
            "Padroneggia 80 abilità.",
            Platinum,
            80
        )
    );
    ladder!(
        "crescita",
        "roster_size",
        (
            "squadra_4",
            "Una squadra",
            "Abbi 4 unità nel roster.",
            Bronze,
            4
        ),
        (
            "squadra_8",
            "Compagnia al completo",
            "Riempi il roster: 8 unità.",
            Silver,
            8
        )
    );
    ladder!(
        "crescita",
        "recruits",
        (
            "reclute_20",
            "Porte aperte",
            "Recluta 20 unità in tutto.",
            Silver,
            20
        )
    );
    add(
        "squadra_alta",
        "Élite",
        "Abbi 4 unità di livello 20 o più.",
        "crescita",
        Gold,
        "elite_units",
        4,
    );

    // ------------------------------------------------------------ abilità e status
    ladder!(
        "arti",
        "ailments",
        (
            "malie_10",
            "Fastidioso",
            "Infliggi 10 status negativi.",
            Bronze,
            10
        ),
        (
            "malie_100",
            "Tormento",
            "Infliggi 100 status negativi.",
            Silver,
            100
        ),
        (
            "malie_1000",
            "Piaga",
            "Infliggi 1000 status negativi.",
            Gold,
            1000
        )
    );
    for (s, name, desc) in [
        (Status::Poison, "Avvelenatore", "Avvelena 25 nemici."),
        (Status::Sleep, "Ninnananna", "Addormenta 25 nemici."),
        (
            Status::Silence,
            "Silenzio in sala",
            "Ammutolisci 25 nemici.",
        ),
        (Status::Slow, "Passo di lumaca", "Rallenta 25 nemici."),
        (Status::Root, "Radici profonde", "Immobilizza 25 nemici."),
        (Status::Burden, "Zavorra", "Appesantisci 25 nemici."),
    ] {
        add(
            &format!("status_{}", s.key()),
            name,
            desc,
            "arti",
            Silver,
            &format!("status:{}", s.key()),
            25,
        );
    }
    for (s, name, desc) in [
        (
            Status::Haste,
            "Più veloce del vento",
            "Dai Rapidità a 25 alleati.",
        ),
        (Status::Aegis, "Scudo vivente", "Dai Egida a 25 alleati."),
        (
            Status::Regen,
            "Primavera",
            "Dai Rigenerazione a 25 alleati.",
        ),
        (Status::Flight, "Ali spiegate", "Dai Volo a 10 alleati."),
    ] {
        let n = if s == Status::Flight { 10 } else { 25 };
        add(
            &format!("dono_{}", s.key()),
            name,
            desc,
            "arti",
            Silver,
            &format!("status:{}", s.key()),
            n,
        );
    }
    ladder!(
        "arti",
        "revives",
        (
            "rianima_1",
            "Non è finita",
            "Rianima un alleato.",
            Bronze,
            1
        ),
        (
            "rianima_25",
            "Seconda possibilità",
            "Rianima 25 alleati.",
            Silver,
            25
        ),
        (
            "rianima_100",
            "Nessuno resta indietro",
            "Rianima 100 alleati.",
            Gold,
            100
        )
    );
    ladder!(
        "arti",
        "items_used",
        (
            "borsa_1",
            "Mano alla borsa",
            "Usa un consumabile in battaglia.",
            Bronze,
            1
        ),
        (
            "borsa_50",
            "Farmacia ambulante",
            "Usa 50 consumabili in battaglia.",
            Silver,
            50
        ),
        (
            "borsa_250",
            "Arsenale tascabile",
            "Usa 250 consumabili in battaglia.",
            Gold,
            250
        )
    );
    ladder!(
        "arti",
        "abilities",
        ("abilita_100", "Repertorio", "Usa 100 abilità.", Bronze, 100),
        (
            "abilita_1000",
            "Virtuoso",
            "Usa 1000 abilità.",
            Silver,
            1000
        ),
        (
            "abilita_5000",
            "Maestro d'armi",
            "Usa 5000 abilità.",
            Gold,
            5000
        )
    );
    add(
        "mp_ridati",
        "Etere",
        "Restituisci 500 MP agli alleati in tutto.",
        "arti",
        Silver,
        "mp_restored",
        500,
    );
    add(
        "resistito",
        "Pelle dura",
        "Le tue unità resistono a 50 status.",
        "arti",
        Silver,
        "resisted",
        50,
    );

    // ------------------------------------------------------------ bottino
    ladder!(
        "bottino",
        "loot",
        (
            "bottino_1",
            "Cosa luccica?",
            "Raccogli un oggetto in battaglia.",
            Bronze,
            1
        ),
        (
            "bottino_25",
            "Raccoglitore",
            "Raccogli 25 oggetti in battaglia.",
            Silver,
            25
        ),
        (
            "bottino_100",
            "Sciacallo",
            "Raccogli 100 oggetti in battaglia.",
            Gold,
            100
        ),
        (
            "bottino_500",
            "Tesoriere",
            "Raccogli 500 oggetti in battaglia.",
            Platinum,
            500
        )
    );
    add(
        "raro_1",
        "Un pezzo raro",
        "Raccogli un oggetto raro.",
        "bottino",
        Bronze,
        "loot:rare",
        1,
    );
    add(
        "epico_1",
        "Epico!",
        "Raccogli un oggetto epico.",
        "bottino",
        Silver,
        "loot:epic",
        1,
    );
    add(
        "leggenda_1",
        "Leggendario",
        "Raccogli un oggetto leggendario.",
        "bottino",
        Gold,
        "loot:legendary",
        1,
    );
    add(
        "leggenda_10",
        "Collezionista di leggende",
        "Raccogli 10 oggetti leggendari.",
        "bottino",
        Platinum,
        "loot:legendary",
        10,
    );
    add(
        "bottino_ricco",
        "Bottino ricco",
        "Raccogli 4 oggetti in una sola partita.",
        "bottino",
        Silver,
        "best_loot_match",
        4,
    );

    // ------------------------------------------------------------ economia
    ladder!(
        "economia",
        "coins_earned",
        (
            "guadagni_1k",
            "Paga di soldato",
            "Guadagna 1.000 monete dalle partite.",
            Bronze,
            1_000
        ),
        (
            "guadagni_10k",
            "Mercenario",
            "Guadagna 10.000 monete dalle partite.",
            Silver,
            10_000
        ),
        (
            "guadagni_100k",
            "Fortuna di guerra",
            "Guadagna 100.000 monete dalle partite.",
            Gold,
            100_000
        )
    );
    ladder!(
        "economia",
        "bought",
        (
            "compra_1",
            "Primo acquisto",
            "Compra un oggetto al negozio.",
            Bronze,
            1
        ),
        (
            "compra_25",
            "Cliente affezionato",
            "Compra 25 oggetti.",
            Silver,
            25
        ),
        (
            "compra_100",
            "Socio del mercante",
            "Compra 100 oggetti.",
            Gold,
            100
        )
    );
    ladder!(
        "economia",
        "sold",
        ("vendi_1", "Svuota zaino", "Vendi un oggetto.", Bronze, 1),
        ("vendi_25", "Robivecchi", "Vendi 25 oggetti.", Silver, 25),
        ("vendi_100", "Mercante", "Vendi 100 oggetti.", Gold, 100)
    );
    ladder!(
        "economia",
        "coins_spent",
        (
            "spesa_2k",
            "Si spende",
            "Spendi 2.000 monete al negozio.",
            Bronze,
            2_000
        ),
        (
            "spesa_20k",
            "Le mani bucate",
            "Spendi 20.000 monete al negozio.",
            Silver,
            20_000
        )
    );
    ladder!(
        "economia",
        "peak_coins",
        (
            "cassa_2k",
            "Gruzzolo",
            "Abbi 2.000 monete insieme.",
            Bronze,
            2_000
        ),
        (
            "cassa_10k",
            "Forziere",
            "Abbi 10.000 monete insieme.",
            Silver,
            10_000
        ),
        (
            "cassa_50k",
            "Drago sull'oro",
            "Abbi 50.000 monete insieme.",
            Gold,
            50_000
        )
    );
    add(
        "inventario_30",
        "Magazzino",
        "Abbi 30 oggetti in inventario.",
        "economia",
        Silver,
        "peak_items",
        30,
    );

    // ------------------------------------------------------------ bottega
    for (k, verb, a, b, c) in [
        (
            "salvage",
            "Smonta",
            "Smontatore",
            "Rottamaio",
            "Nulla si butta",
        ),
        (
            "refine",
            "Raffina",
            "Fabbro",
            "Mastro forgiatore",
            "Fucina eterna",
        ),
        (
            "enchant",
            "Incanta",
            "Apprendista incantatore",
            "Incantatore",
            "Arcimago della forgia",
        ),
        (
            "brew",
            "Distilla",
            "Alambicco",
            "Erborista",
            "Grande alchimista",
        ),
    ] {
        let low = verb.to_lowercase();
        add(
            &format!("{k}_1"),
            a,
            &format!("{verb} un oggetto alla bottega.")
                .replace("Distilla un oggetto", "Distilla un consumabile"),
            "bottega",
            Bronze,
            &format!("craft:{k}"),
            1,
        );
        add(
            &format!("{k}_25"),
            b,
            &format!("{}: 25 volte alla bottega.", verb),
            "bottega",
            Silver,
            &format!("craft:{k}"),
            25,
        );
        add(
            &format!("{k}_100"),
            c,
            &format!("{}: 100 volte alla bottega.", verb),
            "bottega",
            Gold,
            &format!("craft:{k}"),
            100,
        );
        let _ = low;
    }

    // ------------------------------------------------------------ campagna
    ladder!(
        "campagna",
        "campaign_nodes",
        (
            "campagna_1",
            "In cammino",
            "Supera la prima tappa della campagna.",
            Bronze,
            1
        ),
        (
            "campagna_5",
            "Esploratore",
            "Supera 5 tappe della campagna.",
            Silver,
            5
        ),
        (
            "campagna_10",
            "Pellegrino",
            "Supera 10 tappe della campagna.",
            Gold,
            10
        )
    );
    add(
        "campagna_fine",
        "Ai confini del mondo",
        "Supera tutte le tappe della campagna.",
        "campagna",
        Platinum,
        "campaign_done",
        1,
    );
    ladder!(
        "campagna",
        "wins:campaign",
        (
            "campagna_vittorie_25",
            "Cronache",
            "Vinci 25 battaglie di campagna.",
            Silver,
            25
        )
    );
    ladder!(
        "campagna",
        "wins:pve",
        (
            "ia_10",
            "Contro la macchina",
            "Vinci 10 partite contro l'IA.",
            Bronze,
            10
        ),
        (
            "ia_100",
            "La macchina si arrende",
            "Vinci 100 partite contro l'IA.",
            Gold,
            100
        )
    );

    // ------------------------------------------------------------ classificata
    ladder!(
        "pvp",
        "wins:pvp",
        (
            "pvp_1",
            "Sfida accettata",
            "Vinci una partita classificata.",
            Bronze,
            1
        ),
        (
            "pvp_10",
            "Duellante",
            "Vinci 10 partite classificate.",
            Silver,
            10
        ),
        (
            "pvp_50",
            "Gladiatore",
            "Vinci 50 partite classificate.",
            Gold,
            50
        ),
        (
            "pvp_200",
            "Re dell'arena",
            "Vinci 200 partite classificate.",
            Platinum,
            200
        )
    );
    ladder!(
        "pvp",
        "peak_elo",
        (
            "elo_1100",
            "Sopra la media",
            "Raggiungi 1100 di ELO.",
            Bronze,
            1100
        ),
        (
            "elo_1250",
            "Promessa",
            "Raggiungi 1250 di ELO.",
            Silver,
            1250
        ),
        ("elo_1400", "Esperto", "Raggiungi 1400 di ELO.", Gold, 1400),
        (
            "elo_1600",
            "Gran maestro",
            "Raggiungi 1600 di ELO.",
            Platinum,
            1600
        )
    );
    ladder!(
        "pvp",
        "best_streak",
        (
            "serie_3",
            "In serie",
            "Vinci 3 classificate di fila.",
            Bronze,
            3
        ),
        (
            "serie_5",
            "Inarrestabile",
            "Vinci 5 classificate di fila.",
            Silver,
            5
        ),
        (
            "serie_10",
            "Dominio",
            "Vinci 10 classificate di fila.",
            Gold,
            10
        )
    );
    add(
        "davide",
        "Davide e Golia",
        "Batti in classificata chi ha almeno 200 punti di ELO più di te.",
        "pvp",
        Gold,
        "giant_slain",
        1,
    );
    add(
        "stagione_10",
        "Fra i migliori",
        "Chiudi una stagione fra i primi 10.",
        "pvp",
        Gold,
        "season_top10",
        1,
    );
    add(
        "stagione_3",
        "Sul podio",
        "Chiudi una stagione fra i primi 3.",
        "pvp",
        Gold,
        "season_top3",
        1,
    );
    add(
        "stagione_1",
        "Campione della stagione",
        "Chiudi una stagione al primo posto.",
        "pvp",
        Platinum,
        "season_first",
        1,
    );

    // ------------------------------------------------------------ clan
    add(
        "clan_entra",
        "Non più soli",
        "Entra in un clan o fondane uno.",
        "clan",
        Bronze,
        "in_clan",
        1,
    );
    add(
        "clan_fonda",
        "Fondatore",
        "Fonda un clan.",
        "clan",
        Silver,
        "clan_founded",
        1,
    );
    ladder!(
        "clan",
        "wins:coop",
        (
            "coop_1",
            "Fianco a fianco",
            "Vinci una missione cooperativa di clan.",
            Bronze,
            1
        ),
        (
            "coop_10",
            "Compagni d'arme",
            "Vinci 10 missioni cooperative.",
            Silver,
            10
        ),
        (
            "coop_50",
            "Fratellanza",
            "Vinci 50 missioni cooperative.",
            Gold,
            50
        )
    );
    ladder!(
        "clan",
        "coop_difficulty",
        (
            "coop_dura",
            "Missione dura",
            "Vinci una missione cooperativa di difficoltà 3.",
            Silver,
            3
        ),
        (
            "coop_estrema",
            "Missione impossibile",
            "Vinci una missione cooperativa di difficoltà 5.",
            Gold,
            5
        )
    );
    add(
        "caccia_1",
        "Cacciatori di campioni",
        "Abbatti un campione in una missione di caccia.",
        "clan",
        Silver,
        "hunts_won",
        1,
    );
    ladder!(
        "clan",
        "wins:war",
        (
            "guerra_1",
            "Casus belli",
            "Vinci una guerra fra clan.",
            Bronze,
            1
        ),
        (
            "guerra_10",
            "Signore della guerra",
            "Vinci 10 guerre fra clan.",
            Gold,
            10
        )
    );
    add(
        "torneo_gioca",
        "Iscritti",
        "Gioca un incontro di torneo.",
        "clan",
        Bronze,
        "tournament_played",
        1,
    );
    add(
        "torneo_finale",
        "In finale",
        "Arriva in finale in un torneo.",
        "clan",
        Gold,
        "tournament_final",
        1,
    );
    add(
        "torneo_vinto",
        "Coppa alzata",
        "Vinci un torneo.",
        "clan",
        Platinum,
        "tournament_won",
        1,
    );

    // ------------------------------------------------------------ statuti
    ladder!(
        "statuti",
        "clean_wins",
        (
            "pulito_10",
            "Fair play",
            "Vinci 10 partite senza violare gli statuti.",
            Bronze,
            10
        ),
        (
            "pulito_100",
            "Rispettoso delle regole",
            "Vinci 100 partite senza violare gli statuti.",
            Gold,
            100
        )
    );
    add(
        "violazione_1",
        "Fuorilegge",
        "Viola uno statuto. Capita.",
        "statuti",
        Bronze,
        "violations",
        1,
    );
    add(
        "violazione_50",
        "Recidivo",
        "Viola 50 statuti.",
        "statuti",
        Silver,
        "violations",
        50,
    );
    add(
        "espulso",
        "Cartellino rosso",
        "Fatti espellere un'unità dal campo.",
        "statuti",
        Bronze,
        "expelled",
        1,
    );

    // ------------------------------------------------------------ ambienti
    for b in BIOMES {
        add(
            &format!("vinci_{b}"),
            &format!("Conquista: {}", biome_it(b)),
            &format!("Vinci una partita in {}.", biome_it(b).to_lowercase()),
            "ambienti",
            Bronze,
            &format!("biome_win:{b}"),
            1,
        );
    }
    add(
        "ambienti_tutti",
        "Giramondo",
        "Vinci in tutti e cinque gli ambienti.",
        "ambienti",
        Gold,
        "biomes_won",
        5,
    );
    ladder!(
        "ambienti",
        "moves",
        (
            "passi_100",
            "Viandante",
            "Muovi le tue unità 100 volte.",
            Bronze,
            100
        ),
        (
            "passi_1000",
            "Grande marcia",
            "Muovi le tue unità 1000 volte.",
            Silver,
            1000
        ),
        (
            "passi_5000",
            "Il mondo a piedi",
            "Muovi le tue unità 5000 volte.",
            Gold,
            5000
        )
    );
    add(
        "lava",
        "Piedi bollenti",
        "Le tue unità subiscono 500 danni dalla lava.",
        "ambienti",
        Silver,
        "lava_damage",
        500,
    );

    // ------------------------------------------------------------ fedeltà
    ladder!(
        "fedelta",
        "days",
        (
            "giorni_3",
            "Ci si rivede",
            "Gioca in 3 giorni diversi.",
            Bronze,
            3
        ),
        (
            "giorni_7",
            "Una settimana di battaglie",
            "Gioca in 7 giorni diversi.",
            Bronze,
            7
        ),
        (
            "giorni_30",
            "Abitudine",
            "Gioca in 30 giorni diversi.",
            Silver,
            30
        ),
        (
            "giorni_100",
            "Fedelissimo",
            "Gioca in 100 giorni diversi.",
            Gold,
            100
        ),
        (
            "giorni_365",
            "Un anno sul campo",
            "Gioca in 365 giorni diversi.",
            Platinum,
            365
        )
    );
    ladder!(
        "fedelta",
        "achievements",
        (
            "traguardi_25",
            "Collezionista",
            "Sblocca 25 traguardi.",
            Silver,
            25
        ),
        (
            "traguardi_75",
            "Cacciatore di traguardi",
            "Sblocca 75 traguardi.",
            Gold,
            75
        ),
        (
            "traguardi_150",
            "Completista",
            "Sblocca 150 traguardi.",
            Platinum,
            150
        )
    );

    // ------------------------------------------------------------ segreti
    add(
        "cloni",
        "Siamo tutti uguali",
        "Vinci con quattro unità della stessa classe.",
        "segreti",
        Silver,
        "clone_win",
        1,
    );
    add(
        "tutti_diversi",
        "Quattro mestieri",
        "Vinci con quattro unità di quattro classi diverse, nessuna di base.",
        "segreti",
        Silver,
        "advanced_win",
        1,
    );
    add(
        "pacifista",
        "Mani pulite",
        "Vinci una partita in cui le tue unità non mettono KO nessuno.",
        "segreti",
        Gold,
        "pacifist_win",
        1,
    );
    add(
        "solo_cure",
        "Chi cura vince",
        "Cura almeno 1000 HP in una sola partita.",
        "segreti",
        Silver,
        "best_heal_match",
        1000,
    );
    add(
        "mille",
        "Mille in una notte",
        "Infliggi almeno 3000 danni in una sola partita.",
        "segreti",
        Silver,
        "best_damage_match",
        3000,
    );
    add(
        "borsa_vuota",
        "Borsa vuota",
        "Usa 4 consumabili in una sola partita.",
        "segreti",
        Silver,
        "best_items_match",
        4,
    );
    add(
        "sonno_pesante",
        "Dormiglioni",
        "Addormenta 4 nemici in una sola partita.",
        "segreti",
        Gold,
        "best_sleep_match",
        4,
    );
    add(
        "risorto",
        "Lazzaro",
        "Rianima la stessa unità due volte nella stessa partita.",
        "segreti",
        Gold,
        "double_revive",
        1,
    );
    add(
        "rimonta",
        "Rimonta",
        "Vinci dopo aver perso metà della squadra.",
        "segreti",
        Gold,
        "comeback",
        1,
    );
    add(
        "notturno",
        "Nottambulo",
        "Finisci una partita fra le 3 e le 5 del mattino.",
        "segreti",
        Bronze,
        "night_owl",
        1,
    );
    add(
        "tredici",
        "Tredici",
        "Vinci una partita esattamente al turno 13.",
        "segreti",
        Silver,
        "thirteen",
        1,
    );
    add(
        "leggenda_morto",
        "Il tesoro del caduto",
        "Raccogli un oggetto leggendario da un campione.",
        "segreti",
        Gold,
        "champion_legendary",
        1,
    );
    v
}

// ---------------------------------------------------------------- contatori

pub async fn bump(
    conn: &mut SqliteConnection,
    account: i64,
    key: &str,
    delta: i64,
) -> sqlx::Result<()> {
    if delta == 0 {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO counters (account_id, key, value) VALUES (?, ?, ?)
         ON CONFLICT(account_id, key) DO UPDATE SET value = value + excluded.value",
    )
    .bind(account)
    .bind(key)
    .bind(delta)
    .execute(conn)
    .await?;
    Ok(())
}

pub async fn peak(
    conn: &mut SqliteConnection,
    account: i64,
    key: &str,
    value: i64,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO counters (account_id, key, value) VALUES (?, ?, ?)
         ON CONFLICT(account_id, key) DO UPDATE SET value = MAX(value, excluded.value)",
    )
    .bind(account)
    .bind(key)
    .bind(value)
    .execute(conn)
    .await?;
    Ok(())
}

async fn set(conn: &mut SqliteConnection, account: i64, key: &str, value: i64) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO counters (account_id, key, value) VALUES (?, ?, ?)
         ON CONFLICT(account_id, key) DO UPDATE SET value = excluded.value",
    )
    .bind(account)
    .bind(key)
    .bind(value)
    .execute(conn)
    .await?;
    Ok(())
}

pub async fn counters(
    conn: &mut SqliteConnection,
    account: i64,
) -> sqlx::Result<HashMap<String, i64>> {
    Ok(
        sqlx::query_as::<_, (String, i64)>("SELECT key, value FROM counters WHERE account_id = ?")
            .bind(account)
            .fetch_all(conn)
            .await?
            .into_iter()
            .collect(),
    )
}

/// Contatori «vivi», ricalcolati dal database: monete e oggetti posseduti,
/// roster, livelli, padronanze, classi raggiunte, popoli, clan, campagna, ELO.
async fn refresh_live(conn: &mut SqliteConnection, account: i64) -> sqlx::Result<()> {
    let (coins, elo): (i64, i64) = sqlx::query_as("SELECT coins, elo FROM accounts WHERE id = ?")
        .bind(account)
        .fetch_one(&mut *conn)
        .await?;
    peak(conn, account, "peak_coins", coins).await?;
    peak(conn, account, "peak_elo", elo).await?;
    let items: i64 =
        sqlx::query_scalar("SELECT COALESCE(SUM(qty), 0) FROM inventory WHERE account_id = ?")
            .bind(account)
            .fetch_one(&mut *conn)
            .await?;
    peak(conn, account, "peak_items", items).await?;
    let roster: Vec<(String, String, i64)> =
        sqlx::query_as("SELECT class, people, level FROM roster WHERE account_id = ?")
            .bind(account)
            .fetch_all(&mut *conn)
            .await?;
    peak(conn, account, "roster_size", roster.len() as i64).await?;
    peak(
        conn,
        account,
        "max_level",
        roster.iter().map(|r| r.2).max().unwrap_or(0),
    )
    .await?;
    peak(
        conn,
        account,
        "elite_units",
        roster.iter().filter(|r| r.2 >= 20).count() as i64,
    )
    .await?;
    for (class, people, _) in &roster {
        peak(conn, account, &format!("class_reached:{class}"), 1).await?;
        peak(conn, account, &format!("people:{people}"), 1).await?;
    }
    let mastered: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM roster_abilities a JOIN roster r ON r.id = a.roster_id
         WHERE r.account_id = ? AND a.mastered = 1",
    )
    .bind(account)
    .fetch_one(&mut *conn)
    .await?;
    peak(conn, account, "mastered", mastered).await?;
    let in_clan: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM clan_members WHERE account_id = ?)")
            .bind(account)
            .fetch_one(&mut *conn)
            .await?;
    if in_clan {
        peak(conn, account, "in_clan", 1).await?;
    }
    let nodes: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM campaign_progress WHERE account_id = ?")
            .bind(account)
            .fetch_one(&mut *conn)
            .await?;
    peak(conn, account, "campaign_nodes", nodes).await?;
    if nodes as usize >= crate::campaign::total_nodes() {
        peak(conn, account, "campaign_done", 1).await?;
    }
    // Derivati: quante classi raggiunte, quanti popoli, quanti ambienti vinti.
    let c = counters(conn, account).await?;
    let count = |prefix: &str| {
        c.iter()
            .filter(|(k, v)| k.starts_with(prefix) && **v > 0)
            .count() as i64
    };
    set(conn, account, "classes_reached", count("class_reached:")).await?;
    set(conn, account, "peoples_recruited", count("people:")).await?;
    set(conn, account, "biomes_won", count("biome_win:")).await?;
    Ok(())
}

#[derive(Clone, Debug, Serialize)]
pub struct Unlocked {
    pub id: String,
    pub name: String,
    pub tier: Tier,
    pub coins: i64,
}

/// Sblocca i traguardi raggiunti dall'account e dà i premi. Restituisce i nuovi.
pub async fn check(conn: &mut SqliteConnection, account: i64) -> sqlx::Result<Vec<Unlocked>> {
    refresh_live(conn, account).await?;
    // Premi in monete: impostazione `achievement_coins` (pannello admin).
    let paid = sqlx::query_scalar::<_, String>(
        "SELECT value FROM settings WHERE key = 'achievement_coins'",
    )
    .fetch_optional(&mut *conn)
    .await?
    .is_none_or(|v| v != "false");
    let mut out = Vec::new();
    // Due giri: il traguardo «Sblocca N traguardi» dipende dagli altri.
    for _ in 0..2 {
        let have: HashSet<String> = sqlx::query_scalar::<_, String>(
            "SELECT achievement FROM achievements WHERE account_id = ?",
        )
        .bind(account)
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .collect();
        set(conn, account, "achievements", have.len() as i64).await?;
        let c = counters(conn, account).await?;
        let mut new_here = 0;
        for d in catalog() {
            if have.contains(&d.id) || c.get(&d.counter).copied().unwrap_or(0) < d.target {
                continue;
            }
            sqlx::query("INSERT OR IGNORE INTO achievements (account_id, achievement, unlocked_at, seen) VALUES (?, ?, ?, 0)")
                .bind(account)
                .bind(&d.id)
                .bind(now())
                .execute(&mut *conn)
                .await?;
            let coins = if paid { d.tier.coins() } else { 0 };
            sqlx::query("UPDATE accounts SET coins = coins + ? WHERE id = ?")
                .bind(coins)
                .bind(account)
                .execute(&mut *conn)
                .await?;
            if d.tier == Tier::Platinum {
                sqlx::query("INSERT OR IGNORE INTO titles (account_id, title, season_id, awarded_at) VALUES (?, ?, NULL, ?)")
                    .bind(account)
                    .bind(&d.name)
                    .bind(now())
                    .execute(&mut *conn)
                    .await?;
            }
            out.push(Unlocked {
                id: d.id.clone(),
                name: d.name.clone(),
                tier: d.tier,
                coins,
            });
            new_here += 1;
        }
        if new_here == 0 {
            break;
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------- fatti di una partita

/// Cosa serve sapere di una partita per un giocatore umano.
pub struct Seat<'a> {
    pub account: i64,
    pub team: u8,
    pub units: &'a [usize],
    pub result: &'a str,
    pub mode: &'a str,
    pub coins: i64,
    pub elo_before: i64,
    pub opponent_elo: Option<i64>,
    pub ranked: bool,
    pub level_ups: i64,
    pub hunt: bool,
    pub difficulty: u8,
}

/// Rigioca la partita e aggiorna i contatori di un giocatore.
pub async fn after_match(
    conn: &mut SqliteConnection,
    setup: &BattleSetup,
    actions: &[Action],
    battle: &Battle,
    seat: Seat<'_>,
) -> sqlx::Result<()> {
    let acc = seat.account;
    let mine: HashSet<u32> = seat.units.iter().map(|&i| i as u32).collect();
    let win = seat.result == "win";

    // Rigiocata: eventi completi (il motore è deterministico).
    let (mut b, mut events) = Battle::new(setup);
    for a in actions {
        match b.apply(*a) {
            Ok(e) => events.extend(e),
            Err(_) => break,
        }
    }
    let team_of = |u: u32| battle.units.get(u as usize).map(|x| x.team);

    let mut max_hit = 0i64;
    let mut ailments = 0i64;
    let mut statuses: HashMap<&'static str, i64> = HashMap::new();
    let (mut revives, mut items_used, mut abilities, mut moves, mut mp, mut resisted) =
        (0i64, 0i64, 0i64, 0i64, 0i64, 0i64);
    let mut lava = 0i64;
    let mut loot_rarity: HashMap<u32, String> = HashMap::new();
    let mut loot: HashMap<String, i64> = HashMap::new();
    let mut loot_total = 0i64;
    let mut kills_by: HashMap<u32, i64> = HashMap::new();
    let mut revived: HashMap<u32, i64> = HashMap::new();
    let mut sleeps = 0i64;
    let mut expelled = 0i64;
    // Ultimo attaccante di ogni unità e KO per azione.
    let mut last_by: HashMap<u32, u32> = HashMap::new();
    let mut action_kills = 0i64;
    let mut best_action_kills = 0i64;
    let mut lost_mine = 0i64;
    let mut worst_lost = 0i64;
    let mut champion_legendary = false;
    let champion = match setup.objective {
        tactica_core::Objective::Champion(c) => Some(c),
        tactica_core::Objective::Rout => None,
    };
    for e in &events {
        match e {
            Event::TurnStarted { .. } | Event::AbilityUsed { .. } | Event::ItemUsed { .. } => {
                best_action_kills = best_action_kills.max(action_kills);
                action_kills = 0;
                if let Event::AbilityUsed { unit, .. } = e
                    && mine.contains(unit)
                {
                    abilities += 1;
                }
                if let Event::ItemUsed { unit, .. } = e
                    && mine.contains(unit)
                {
                    items_used += 1;
                }
            }
            Event::Moved { unit, .. } if mine.contains(unit) => moves += 1,
            Event::Damaged {
                unit, by, amount, ..
            } => {
                last_by.insert(*unit, *by);
                if mine.contains(by) && team_of(*unit) != Some(seat.team) {
                    max_hit = max_hit.max(*amount as i64);
                }
            }
            Event::Defeated { unit } => {
                if mine.contains(unit) {
                    lost_mine += 1;
                    worst_lost = worst_lost.max(lost_mine);
                } else if let Some(by) = last_by.get(unit)
                    && mine.contains(by)
                    && team_of(*unit) != Some(seat.team)
                {
                    *kills_by.entry(*by).or_default() += 1;
                    action_kills += 1;
                }
            }
            Event::Revived { unit, by, .. } => {
                if mine.contains(by) {
                    revives += 1;
                }
                if mine.contains(unit) {
                    lost_mine -= 1;
                    *revived.entry(*unit).or_default() += 1;
                }
            }
            Event::MpRestored { by, amount, .. } if mine.contains(by) => mp += *amount as i64,
            Event::LootDropped { unit, rarity, .. } => {
                loot_rarity.insert(*unit, format!("{rarity:?}").to_lowercase());
            }
            Event::LootPicked { unit, from, .. } if mine.contains(unit) => {
                loot_total += 1;
                let r = loot_rarity
                    .get(from)
                    .cloned()
                    .unwrap_or_else(|| "common".into());
                if r == "legendary" && Some(*from) == champion {
                    champion_legendary = true;
                }
                *loot.entry(r).or_default() += 1;
            }
            Event::TerrainDamage {
                unit,
                amount,
                terrain,
                ..
            } if mine.contains(unit) => {
                if format!("{terrain:?}") == "Lava" {
                    lava += *amount as i64;
                }
            }
            Event::StatusApplied { unit, status } => {
                // Chi l'ha dato non è nell'evento: conta lo schieramento del bersaglio.
                let enemy = team_of(*unit) != Some(seat.team);
                if enemy && status.is_negative() {
                    ailments += 1;
                    *statuses.entry(status.key()).or_default() += 1;
                    if *status == Status::Sleep {
                        sleeps += 1;
                    }
                } else if !enemy && !status.is_negative() {
                    *statuses.entry(status.key()).or_default() += 1;
                }
            }
            Event::StatusResisted { unit, .. } if mine.contains(unit) => resisted += 1,
            Event::Expelled { unit } if mine.contains(unit) => expelled += 1,
            _ => {}
        }
    }
    best_action_kills = best_action_kills.max(action_kills);

    // Dalla partita finale: le mie unità.
    let my_units: Vec<&tactica_core::unit::Unit> = seat
        .units
        .iter()
        .filter_map(|&i| battle.units.get(i))
        .collect();
    let survivors = my_units
        .iter()
        .filter(|u| u.condition == Condition::Active)
        .count();
    let kills: i64 = my_units.iter().map(|u| u.tally.kills as i64).sum();
    let damage: i64 = my_units.iter().map(|u| u.tally.damage_dealt as i64).sum();
    let healing: i64 = my_units.iter().map(|u| u.tally.healing_done as i64).sum();
    let violations: i64 = my_units.iter().map(|u| u.tally.violations as i64).sum();
    let turns = battle.turn() as i64;

    bump(conn, acc, "matches", 1).await?;
    match seat.result {
        "win" => bump(conn, acc, "wins", 1).await?,
        "loss" => bump(conn, acc, "losses", 1).await?,
        _ => bump(conn, acc, "draws", 1).await?,
    }
    if win {
        bump(conn, acc, &format!("wins:{}", seat.mode), 1).await?;
    }
    bump(conn, acc, "kills", kills).await?;
    bump(conn, acc, "damage", damage).await?;
    bump(conn, acc, "healing", healing).await?;
    bump(conn, acc, "violations", violations).await?;
    bump(conn, acc, "coins_earned", seat.coins.max(0)).await?;
    bump(conn, acc, "level_ups", seat.level_ups).await?;
    bump(conn, acc, "ailments", ailments).await?;
    for (k, n) in &statuses {
        bump(conn, acc, &format!("status:{k}"), *n).await?;
    }
    bump(conn, acc, "revives", revives).await?;
    bump(conn, acc, "items_used", items_used).await?;
    bump(conn, acc, "abilities", abilities).await?;
    bump(conn, acc, "moves", moves).await?;
    bump(conn, acc, "mp_restored", mp).await?;
    bump(conn, acc, "resisted", resisted).await?;
    bump(conn, acc, "lava_damage", lava).await?;
    bump(conn, acc, "expelled", expelled).await?;
    bump(conn, acc, "loot", loot_total).await?;
    for (r, n) in &loot {
        bump(conn, acc, &format!("loot:{r}"), *n).await?;
    }
    peak(conn, acc, "max_hit", max_hit).await?;
    peak(
        conn,
        acc,
        "best_kills_unit",
        kills_by.values().copied().max().unwrap_or(0),
    )
    .await?;
    peak(conn, acc, "multi_kill", best_action_kills).await?;
    peak(conn, acc, "best_loot_match", loot_total).await?;
    peak(conn, acc, "best_heal_match", healing).await?;
    peak(conn, acc, "best_damage_match", damage).await?;
    peak(conn, acc, "best_items_match", items_used).await?;
    peak(conn, acc, "best_sleep_match", sleeps).await?;
    if revived.values().any(|&n| n >= 2) {
        peak(conn, acc, "double_revive", 1).await?;
    }
    if champion_legendary {
        peak(conn, acc, "champion_legendary", 1).await?;
    }
    if turns >= 200 {
        bump(conn, acc, "long_matches", 1).await?;
    }
    // Ora del server: fra le 3 e le 5 (ora italiana approssimata: UTC+1).
    let hour = ((now() / 3600) + 1).rem_euclid(24);
    if (3..5).contains(&hour) {
        peak(conn, acc, "night_owl", 1).await?;
    }

    // Giorni diversi di gioco.
    let today = now().div_euclid(86_400);
    let last: Option<i64> =
        sqlx::query_scalar("SELECT value FROM counters WHERE account_id = ? AND key = 'last_day'")
            .bind(acc)
            .fetch_optional(&mut *conn)
            .await?;
    if last != Some(today) {
        bump(conn, acc, "days", 1).await?;
        set(conn, acc, "last_day", today).await?;
    }

    if win {
        let size = my_units.len();
        if survivors == size {
            bump(conn, acc, "flawless", 1).await?;
        }
        if survivors == 1 && size > 1 {
            bump(conn, acc, "clutch", 1).await?;
        }
        if turns <= 30 {
            bump(conn, acc, "quick_wins", 1).await?;
        }
        if turns == 13 {
            peak(conn, acc, "thirteen", 1).await?;
        }
        if violations == 0 {
            bump(conn, acc, "clean_wins", 1).await?;
        }
        if kills == 0 {
            peak(conn, acc, "pacifist_win", 1).await?;
        }
        if size >= 2 && worst_lost * 2 >= size as i64 {
            peak(conn, acc, "comeback", 1).await?;
        }
        let biome = format!("{:?}", battle.map.biome).to_lowercase();
        peak(conn, acc, &format!("biome_win:{biome}"), 1).await?;
        let classes: HashSet<Class> = my_units.iter().map(|u| u.class).collect();
        for c in &classes {
            bump(conn, acc, &format!("class_win:{}", c.key()), 1).await?;
        }
        if size >= 4 && classes.len() == 1 {
            peak(conn, acc, "clone_win", 1).await?;
        }
        if size >= 4 && classes.len() == size && classes.iter().all(|c| !c.is_base()) {
            peak(conn, acc, "advanced_win", 1).await?;
        }
        let peoples: HashSet<People> = my_units.iter().map(|u| u.people).collect();
        if size >= 3 && peoples.len() == 1 {
            let p = *peoples.iter().next().unwrap();
            peak(conn, acc, &format!("pure_win:{}", p.key()), 1).await?;
        }
        if size >= 4 && peoples.len() >= 4 {
            peak(conn, acc, "mixed_win", 1).await?;
        }
        if seat.mode == "coop" || seat.mode == "campaign" {
            peak(
                conn,
                acc,
                "coop_difficulty",
                if seat.mode == "coop" {
                    seat.difficulty as i64
                } else {
                    0
                },
            )
            .await?;
            if seat.hunt {
                bump(conn, acc, "hunts_won", 1).await?;
            }
        }
        if seat.ranked
            && let Some(o) = seat.opponent_elo
            && o - seat.elo_before >= 200
        {
            peak(conn, acc, "giant_slain", 1).await?;
        }
    }
    // Serie di vittorie in classificata.
    if seat.ranked {
        if win {
            bump(conn, acc, "streak", 1).await?;
            let s: i64 = sqlx::query_scalar(
                "SELECT value FROM counters WHERE account_id = ? AND key = 'streak'",
            )
            .bind(acc)
            .fetch_one(&mut *conn)
            .await?;
            peak(conn, acc, "best_streak", s).await?;
        } else {
            set(conn, acc, "streak", 0).await?;
        }
    }
    if seat.mode == "war" {
        // Gli incontri di torneo sono guerre: li segna `tournaments`.
    }
    Ok(())
}

// ---------------------------------------------------------------- API

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/achievements", get(public_catalog))
        .route("/api/me/achievements", get(mine))
        .route("/api/me/achievements/seen", post(seen))
        .route("/api/players/{name}/achievements", get(of_player))
}

#[derive(Serialize)]
struct Entry {
    id: String,
    name: String,
    desc: String,
    category: &'static str,
    tier: Tier,
    secret: bool,
    target: i64,
    progress: i64,
    unlocked_at: Option<i64>,
    /// Percentuale di giocatori che l'hanno sbloccato.
    rarity: f64,
}

#[derive(Serialize)]
struct Book {
    categories: Vec<(&'static str, &'static str)>,
    unlocked: usize,
    total: usize,
    coins: i64,
    entries: Vec<Entry>,
}

async fn rarity(conn: &mut SqliteConnection) -> sqlx::Result<HashMap<String, f64>> {
    let players: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts")
        .fetch_one(&mut *conn)
        .await?;
    let rows: Vec<(String, i64)> =
        sqlx::query_as("SELECT achievement, COUNT(*) FROM achievements GROUP BY achievement")
            .fetch_all(&mut *conn)
            .await?;
    Ok(rows
        .into_iter()
        .map(|(k, n)| (k, n as f64 * 100.0 / players.max(1) as f64))
        .collect())
}

async fn book(
    conn: &mut SqliteConnection,
    account: Option<i64>,
    reveal: bool,
) -> sqlx::Result<Book> {
    let rar = rarity(conn).await?;
    let (c, got): (HashMap<String, i64>, HashMap<String, i64>) = match account {
        Some(a) => (
            counters(conn, a).await?,
            sqlx::query_as::<_, (String, i64)>(
                "SELECT achievement, unlocked_at FROM achievements WHERE account_id = ?",
            )
            .bind(a)
            .fetch_all(&mut *conn)
            .await?
            .into_iter()
            .collect(),
        ),
        None => (HashMap::new(), HashMap::new()),
    };
    let mut coins = 0;
    let entries: Vec<Entry> = catalog()
        .iter()
        .map(|d| {
            let at = got.get(&d.id).copied();
            if at.is_some() {
                coins += d.tier.coins();
            }
            let hide = d.secret && at.is_none() && !reveal;
            Entry {
                id: d.id.clone(),
                name: if hide { "???".into() } else { d.name.clone() },
                desc: if hide {
                    "Traguardo segreto: scoprilo giocando.".into()
                } else {
                    d.desc.clone()
                },
                category: d.category,
                tier: d.tier,
                secret: d.secret,
                target: d.target,
                progress: if hide {
                    0
                } else {
                    c.get(&d.counter).copied().unwrap_or(0).min(d.target)
                },
                unlocked_at: at,
                rarity: rar.get(&d.id).copied().unwrap_or(0.0),
            }
        })
        .collect();
    Ok(Book {
        categories: CATEGORIES.to_vec(),
        unlocked: got.len(),
        total: catalog().len(),
        coins,
        entries,
    })
}

async fn public_catalog(State(s): State<AppState>) -> ApiResult<Json<Book>> {
    let mut conn = s.db.acquire().await?;
    Ok(Json(book(&mut conn, None, false).await?))
}

async fn mine(State(s): State<AppState>, user: AuthUser) -> ApiResult<Json<Book>> {
    let mut tx = s.db.begin().await?;
    check(&mut tx, user.id).await?;
    tx.commit().await?;
    let mut conn = s.db.acquire().await?;
    Ok(Json(book(&mut conn, Some(user.id), false).await?))
}

async fn of_player(State(s): State<AppState>, Path(name): Path<String>) -> ApiResult<Json<Book>> {
    let id: i64 = sqlx::query_scalar("SELECT id FROM accounts WHERE username = ?")
        .bind(&name)
        .fetch_optional(&s.db)
        .await?
        .ok_or_else(|| ApiError::not_found("giocatore inesistente"))?;
    let mut conn = s.db.acquire().await?;
    Ok(Json(book(&mut conn, Some(id), false).await?))
}

async fn seen(State(s): State<AppState>, user: AuthUser) -> ApiResult<()> {
    sqlx::query("UPDATE achievements SET seen = 1 WHERE account_id = ? AND seen = 0")
        .bind(user.id)
        .execute(&s.db)
        .await?;
    Ok(())
}

/// Traguardi sbloccati e non ancora mostrati (per `/api/me`).
pub async fn unseen(conn: &mut SqliteConnection, account: i64) -> sqlx::Result<Vec<Unlocked>> {
    let ids: Vec<String> =
        sqlx::query_scalar("SELECT achievement FROM achievements WHERE account_id = ? AND seen = 0 ORDER BY unlocked_at")
            .bind(account)
            .fetch_all(conn)
            .await?;
    Ok(ids
        .into_iter()
        .filter_map(|id| catalog().iter().find(|d| d.id == id))
        .map(|d| Unlocked {
            id: d.id.clone(),
            name: d.name.clone(),
            tier: d.tier,
            coins: d.tier.coins(),
        })
        .collect())
}

/// Aggiorna contatori (`bump`) e controlla gli sblocchi dopo un'azione fuori
/// dalla battaglia (negozio, bottega, reclute, clan...). Un errore qui non deve
/// far fallire l'azione del giocatore: si registra e basta.
pub async fn record(db: &sqlx::SqlitePool, account: i64, bumps: &[(&str, i64)]) {
    let res: sqlx::Result<()> = async {
        let mut tx = db.begin().await?;
        for (k, n) in bumps {
            bump(&mut tx, account, k, *n).await?;
        }
        check(&mut tx, account).await?;
        tx.commit().await
    }
    .await;
    if let Err(e) = res {
        tracing::error!(account, "traguardi: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_large_and_consistent() {
        let cat = catalog();
        assert!(cat.len() >= 120, "solo {} traguardi", cat.len());
        let ids: HashSet<&str> = cat.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids.len(), cat.len(), "identificativi duplicati");
        for (k, _) in CATEGORIES {
            assert!(cat.iter().any(|d| d.category == k), "categoria vuota: {k}");
        }
        // Ogni contatore usato da un traguardo è alimentato da qualche parte
        // (`src` contiene i sorgenti che fanno bump/peak/set).
        let src = [
            include_str!("achievements.rs"),
            include_str!("api.rs"),
            include_str!("armory.rs"),
            include_str!("craft.rs"),
            include_str!("clans.rs"),
            include_str!("stats.rs"),
            include_str!("seasons.rs"),
            include_str!("tournaments.rs"),
        ]
        .concat();
        for d in cat {
            let base = d.counter.split(':').next().unwrap();
            let fed = src.contains(&format!("\"{}\"", d.counter))
                || src.contains(&format!("\"{base}:"))
                || src.contains(&format!("\"{base}:{{"));
            assert!(fed, "contatore mai aggiornato: {}", d.counter);
        }
    }
}

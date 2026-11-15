-- Account e sessioni
CREATE TABLE accounts (
    id            INTEGER PRIMARY KEY,
    username      TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    password_hash TEXT    NOT NULL,
    elo           INTEGER NOT NULL DEFAULT 1000,
    created_at    INTEGER NOT NULL
);

-- Il token non è salvato in chiaro: si conserva il suo SHA-256.
CREATE TABLE sessions (
    token_hash  TEXT    PRIMARY KEY,
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    expires_at  INTEGER NOT NULL
);
CREATE INDEX sessions_account ON sessions(account_id);

-- Personaggi persistenti di ogni giocatore
CREATE TABLE roster (
    id          INTEGER PRIMARY KEY,
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    name        TEXT    NOT NULL,
    class       TEXT    NOT NULL,
    level       INTEGER NOT NULL DEFAULT 1,
    xp          INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL
);
CREATE INDEX roster_account ON roster(account_id);

-- Partite concluse: seed + setup + azioni = replay completo e verificabile
CREATE TABLE matches (
    id           INTEGER PRIMARY KEY,
    mode         TEXT    NOT NULL,
    seed         INTEGER NOT NULL,
    setup        TEXT    NOT NULL,
    actions      TEXT    NOT NULL,
    finished     INTEGER NOT NULL,
    winner_team  INTEGER,
    turns        INTEGER NOT NULL,
    biome        TEXT    NOT NULL,
    final_hash   TEXT    NOT NULL,
    started_at   INTEGER NOT NULL,
    ended_at     INTEGER NOT NULL
);
CREATE INDEX matches_ended ON matches(ended_at);

CREATE TABLE match_statutes (
    match_id  INTEGER NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
    statute   TEXT    NOT NULL
);
CREATE INDEX match_statutes_statute ON match_statutes(statute);

CREATE TABLE match_players (
    match_id    INTEGER NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
    team        INTEGER NOT NULL,
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    result      TEXT    NOT NULL CHECK (result IN ('win', 'loss', 'draw')),
    elo_before  INTEGER NOT NULL,
    elo_after   INTEGER NOT NULL,
    PRIMARY KEY (match_id, team)
);
CREATE INDEX match_players_account ON match_players(account_id);

-- Una riga per unità schierata: base delle statistiche personali e globali
CREATE TABLE match_units (
    match_id      INTEGER NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
    unit          INTEGER NOT NULL,
    team          INTEGER NOT NULL,
    account_id    INTEGER REFERENCES accounts(id) ON DELETE SET NULL,
    roster_id     INTEGER REFERENCES roster(id) ON DELETE SET NULL,
    class         TEXT    NOT NULL,
    level_start   INTEGER NOT NULL,
    level_end     INTEGER NOT NULL,
    survived      INTEGER NOT NULL,
    damage_dealt  INTEGER NOT NULL,
    damage_taken  INTEGER NOT NULL,
    healing_done  INTEGER NOT NULL,
    kills         INTEGER NOT NULL,
    actions       INTEGER NOT NULL,
    misses        INTEGER NOT NULL,
    violations    INTEGER NOT NULL,
    xp_gained     INTEGER NOT NULL,
    PRIMARY KEY (match_id, unit)
);
CREATE INDEX match_units_account ON match_units(account_id);
CREATE INDEX match_units_class ON match_units(class);

-- Stagioni classificate: classifica propria (ELO stagionale), premi e titoli a fine stagione.
CREATE TABLE seasons (
    id          INTEGER PRIMARY KEY,
    number      INTEGER NOT NULL UNIQUE,
    name        TEXT    NOT NULL,
    starts_at   INTEGER NOT NULL,
    ends_at     INTEGER NOT NULL,
    closed      INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE season_ratings (
    season_id   INTEGER NOT NULL REFERENCES seasons(id) ON DELETE CASCADE,
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    rating      INTEGER NOT NULL,
    peak        INTEGER NOT NULL,
    wins        INTEGER NOT NULL DEFAULT 0,
    losses      INTEGER NOT NULL DEFAULT 0,
    draws       INTEGER NOT NULL DEFAULT 0,
    final_rank  INTEGER,
    PRIMARY KEY (season_id, account_id)
);
CREATE INDEX season_ratings_rank ON season_ratings(season_id, rating DESC);

-- Clan: un account appartiene al massimo a un clan.
CREATE TABLE clans (
    id          INTEGER PRIMARY KEY,
    name        TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    tag         TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    motto       TEXT    NOT NULL DEFAULT '',
    open        INTEGER NOT NULL DEFAULT 1,
    created_at  INTEGER NOT NULL
);

CREATE TABLE clan_members (
    clan_id     INTEGER NOT NULL REFERENCES clans(id) ON DELETE CASCADE,
    account_id  INTEGER NOT NULL UNIQUE REFERENCES accounts(id) ON DELETE CASCADE,
    role        TEXT    NOT NULL CHECK (role IN ('leader', 'officer', 'member')),
    joined_at   INTEGER NOT NULL,
    PRIMARY KEY (clan_id, account_id)
);

CREATE TABLE clan_invites (
    clan_id     INTEGER NOT NULL REFERENCES clans(id) ON DELETE CASCADE,
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    created_at  INTEGER NOT NULL,
    PRIMARY KEY (clan_id, account_id)
);

CREATE TABLE clan_season_points (
    season_id   INTEGER NOT NULL REFERENCES seasons(id) ON DELETE CASCADE,
    clan_id     INTEGER NOT NULL REFERENCES clans(id) ON DELETE CASCADE,
    points      INTEGER NOT NULL DEFAULT 0,
    wins        INTEGER NOT NULL DEFAULT 0,
    final_rank  INTEGER,
    PRIMARY KEY (season_id, clan_id)
);

CREATE TABLE titles (
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    title       TEXT    NOT NULL,
    season_id   INTEGER REFERENCES seasons(id) ON DELETE SET NULL,
    awarded_at  INTEGER NOT NULL,
    PRIMARY KEY (account_id, title)
);

ALTER TABLE match_players ADD COLUMN season_id INTEGER;
ALTER TABLE match_players ADD COLUMN clan_id INTEGER;
ALTER TABLE match_players ADD COLUMN season_before INTEGER;
ALTER TABLE match_players ADD COLUMN season_after INTEGER;

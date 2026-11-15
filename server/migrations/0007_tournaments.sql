-- Tornei fra clan a eliminazione diretta: ogni incontro è una guerra fra clan.
CREATE TABLE tournaments (
    id            INTEGER PRIMARY KEY,
    name          TEXT    NOT NULL,
    creator_clan  INTEGER REFERENCES clans(id) ON DELETE SET NULL,
    max_clans     INTEGER NOT NULL,
    status        TEXT    NOT NULL CHECK (status IN ('signup', 'running', 'done', 'cancelled')),
    winner_clan   INTEGER REFERENCES clans(id) ON DELETE SET NULL,
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL
);

CREATE TABLE tournament_entries (
    tournament_id  INTEGER NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    clan_id        INTEGER NOT NULL REFERENCES clans(id) ON DELETE CASCADE,
    seed           INTEGER,
    joined_at      INTEGER NOT NULL,
    PRIMARY KEY (tournament_id, clan_id)
);

-- Tabellone: round 1 = primo turno; slot = posizione nel round.
-- clan_b NULL = turno libero (bye) per clan_a.
CREATE TABLE tournament_matches (
    tournament_id  INTEGER NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    round          INTEGER NOT NULL,
    slot           INTEGER NOT NULL,
    clan_a         INTEGER REFERENCES clans(id) ON DELETE SET NULL,
    clan_b         INTEGER REFERENCES clans(id) ON DELETE SET NULL,
    war_id         INTEGER REFERENCES clan_wars(id) ON DELETE SET NULL,
    winner_clan    INTEGER REFERENCES clans(id) ON DELETE SET NULL,
    PRIMARY KEY (tournament_id, round, slot)
);

ALTER TABLE clan_wars ADD COLUMN tournament_id INTEGER REFERENCES tournaments(id) ON DELETE SET NULL;

-- Guerre fra clan: ELO di clan e storico delle sfide.
ALTER TABLE clans ADD COLUMN rating INTEGER NOT NULL DEFAULT 1000;
ALTER TABLE clans ADD COLUMN war_wins INTEGER NOT NULL DEFAULT 0;
ALTER TABLE clans ADD COLUMN war_losses INTEGER NOT NULL DEFAULT 0;
ALTER TABLE clans ADD COLUMN war_draws INTEGER NOT NULL DEFAULT 0;

CREATE TABLE clan_wars (
    id                 INTEGER PRIMARY KEY,
    challenger_id      INTEGER NOT NULL REFERENCES clans(id) ON DELETE CASCADE,
    defender_id        INTEGER NOT NULL REFERENCES clans(id) ON DELETE CASCADE,
    status             TEXT    NOT NULL CHECK (status IN ('pending', 'accepted', 'live', 'done', 'declined', 'cancelled')),
    created_at         INTEGER NOT NULL,
    updated_at         INTEGER NOT NULL,
    match_id           INTEGER REFERENCES matches(id) ON DELETE SET NULL,
    winner_id          INTEGER REFERENCES clans(id) ON DELETE SET NULL,
    challenger_before  INTEGER,
    challenger_after   INTEGER,
    defender_before    INTEGER,
    defender_after     INTEGER
);
CREATE INDEX clan_wars_challenger ON clan_wars(challenger_id, status);
CREATE INDEX clan_wars_defender ON clan_wars(defender_id, status);

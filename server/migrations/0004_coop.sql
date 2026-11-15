-- Missioni cooperative: più giocatori nella stessa squadra, quindi la chiave
-- di match_players diventa (partita, account) invece di (partita, squadra).
CREATE TABLE match_players_v2 (
    match_id       INTEGER NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
    team           INTEGER NOT NULL,
    account_id     INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    result         TEXT    NOT NULL CHECK (result IN ('win', 'loss', 'draw')),
    elo_before     INTEGER NOT NULL,
    elo_after      INTEGER NOT NULL,
    coins          INTEGER NOT NULL DEFAULT 0,
    loot           TEXT,
    season_id      INTEGER,
    clan_id        INTEGER,
    season_before  INTEGER,
    season_after   INTEGER,
    PRIMARY KEY (match_id, account_id)
);
INSERT INTO match_players_v2
    SELECT match_id, team, account_id, result, elo_before, elo_after, coins, loot,
           season_id, clan_id, season_before, season_after
    FROM match_players;
DROP TABLE match_players;
ALTER TABLE match_players_v2 RENAME TO match_players;
CREATE INDEX match_players_account ON match_players(account_id);

-- Nome della missione (solo partite cooperative).
ALTER TABLE matches ADD COLUMN mission TEXT;

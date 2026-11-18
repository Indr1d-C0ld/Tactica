-- Traguardi: contatori per account (soglie del catalogo in achievements.rs)
-- e traguardi sbloccati, con il segno «già mostrato» per gli avvisi.
CREATE TABLE counters (
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    key         TEXT    NOT NULL,
    value       INTEGER NOT NULL,
    PRIMARY KEY (account_id, key)
);

CREATE TABLE achievements (
    account_id   INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    achievement  TEXT    NOT NULL,
    unlocked_at  INTEGER NOT NULL,
    seen         INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (account_id, achievement)
);
CREATE INDEX achievements_id ON achievements(achievement);

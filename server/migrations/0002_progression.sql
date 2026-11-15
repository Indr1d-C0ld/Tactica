-- Progressione: monete, inventario, equipaggiamento, risonanza delle abilità
ALTER TABLE accounts ADD COLUMN coins INTEGER NOT NULL DEFAULT 400;

-- Oggetti equipaggiati: lista JSON di chiavi oggetto (fuori dall'inventario).
ALTER TABLE roster ADD COLUMN equipment TEXT NOT NULL DEFAULT '[]';

CREATE TABLE roster_abilities (
    roster_id  INTEGER NOT NULL REFERENCES roster(id) ON DELETE CASCADE,
    ability    TEXT    NOT NULL,
    rp         INTEGER NOT NULL DEFAULT 0,
    mastered   INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (roster_id, ability)
);

-- Oggetti posseduti e non equipaggiati.
CREATE TABLE inventory (
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    item        TEXT    NOT NULL,
    qty         INTEGER NOT NULL CHECK (qty >= 0),
    PRIMARY KEY (account_id, item)
);

ALTER TABLE match_players ADD COLUMN coins INTEGER NOT NULL DEFAULT 0;
ALTER TABLE match_players ADD COLUMN loot TEXT;

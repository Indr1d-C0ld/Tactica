-- Campagna in singolo: tappe vinte da ogni account.
CREATE TABLE campaign_progress (
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    node        TEXT    NOT NULL,
    cleared_at  INTEGER NOT NULL,
    turns       INTEGER NOT NULL,
    PRIMARY KEY (account_id, node)
);

-- Amministrazione: ruolo admin, sospensioni, ultimo accesso, impostazioni
-- di gioco modificabili a caldo e registro delle azioni degli admin.
ALTER TABLE accounts ADD COLUMN is_admin INTEGER NOT NULL DEFAULT 0;
-- Fine della sospensione (secondi Unix); NULL = account attivo.
ALTER TABLE accounts ADD COLUMN banned_until INTEGER;
ALTER TABLE accounts ADD COLUMN ban_reason TEXT;
ALTER TABLE accounts ADD COLUMN last_seen INTEGER;

CREATE TABLE settings (
    key    TEXT PRIMARY KEY,
    value  TEXT NOT NULL
);

CREATE TABLE admin_log (
    id        INTEGER PRIMARY KEY,
    at        INTEGER NOT NULL,
    admin_id  INTEGER REFERENCES accounts(id) ON DELETE SET NULL,
    admin     TEXT    NOT NULL,
    action    TEXT    NOT NULL,
    target    TEXT,
    detail    TEXT
);
CREATE INDEX admin_log_at ON admin_log(at);

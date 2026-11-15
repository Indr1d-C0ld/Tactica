-- Materiali dei mestieri (smantellamento, forgia, alchimia).
CREATE TABLE materials (
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    material TEXT NOT NULL,
    qty INTEGER NOT NULL DEFAULT 0 CHECK (qty >= 0),
    PRIMARY KEY (account_id, material)
);

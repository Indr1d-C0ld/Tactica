-- Versione delle regole del motore con cui è stata giocata la partita:
-- un replay si riproduce solo con la stessa versione. Le partite precedenti
-- all'introduzione degli status alterati sono della versione 1.
ALTER TABLE matches ADD COLUMN rules_version INTEGER NOT NULL DEFAULT 1;

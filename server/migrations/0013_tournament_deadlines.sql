-- Scadenze degli incontri di torneo: alla scadenza un incontro non giocato
-- si assegna a tavolino (vedi tournaments::expire_due).
ALTER TABLE tournaments ADD COLUMN match_hours INTEGER NOT NULL DEFAULT 72;
ALTER TABLE tournament_matches ADD COLUMN deadline INTEGER;
-- Come si è deciso l'incontro: NULL = giocato (o turno libero),
-- 'forfeit' = a tavolino per scadenza, 'admin' = assegnato da un amministratore.
ALTER TABLE tournament_matches ADD COLUMN decided_by TEXT;

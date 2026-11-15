-- Borsa di battaglia: consumabili portati in partita da ogni unità del roster.
ALTER TABLE roster ADD COLUMN bag TEXT NOT NULL DEFAULT '[]';

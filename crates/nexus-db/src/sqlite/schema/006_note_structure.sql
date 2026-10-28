
-- Titel: ein Satz, was auffällt (leer bei älteren Notizen)
ALTER TABLE Notes ADD COLUMN Title TEXT NOT NULL DEFAULT '';
-- Wie oft der Befund aufgefallen ist, und bei welchen weiteren Tickets (kommagetrennt)
ALTER TABLE Notes ADD COLUMN SeenCount INTEGER NOT NULL DEFAULT 1;
ALTER TABLE Notes ADD COLUMN SeenIn TEXT NOT NULL DEFAULT '';

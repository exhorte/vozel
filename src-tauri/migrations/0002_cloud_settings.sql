-- Réglages cloud (Spec_Backend_Desktop.md §2.4) : fournisseur ASR cloud
-- choisi et clé API "BYO" (jamais loguée en clair — voir storage::settings).
-- Colonnes ajoutées à la ligne unique `settings` (id = 1).
ALTER TABLE settings ADD COLUMN cloud_provider TEXT NOT NULL DEFAULT 'groq';
ALTER TABLE settings ADD COLUMN cloud_api_key  TEXT NOT NULL DEFAULT '';

-- Langue de l'interface (FR/EN), demande utilisateur — jusqu'ici toute
-- l'UI était codée en dur en français. Défaut "en" : nouvelle installation
-- ET lignes déjà existantes basculent en anglais (demande explicite de
-- l'utilisateur, pas seulement les nouvelles installations).
ALTER TABLE settings ADD COLUMN ui_language TEXT NOT NULL DEFAULT 'en';

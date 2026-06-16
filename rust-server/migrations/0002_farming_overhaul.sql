-- RuneScape-style farming overhaul: harvest lives, disease/health, compost.
-- Existing rows backfill with sane defaults. Patch ids changed in the locations
-- rewrite, so any in-progress crops are dropped (they would be skipped on restore
-- anyway). This is acceptable for the current beta.

ALTER TABLE farming_patches ADD COLUMN lives_remaining INTEGER NOT NULL DEFAULT 1;
ALTER TABLE farming_patches ADD COLUMN health TEXT NOT NULL DEFAULT 'healthy';
ALTER TABLE farming_patches ADD COLUMN composted INTEGER NOT NULL DEFAULT 0;
ALTER TABLE farming_patches ADD COLUMN disease_cycle_marker INTEGER NOT NULL DEFAULT 0;

DELETE FROM farming_patches;

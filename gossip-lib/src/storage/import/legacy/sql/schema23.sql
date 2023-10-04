ALTER TABLE person_relay DROP COLUMN last_suggested_kind2;
ALTER TABLE person_relay ADD COLUMN manually_paired_read INTEGER NOT NULL DEFAULT 0;
ALTER TABLE person_relay ADD COLUMN manually_paired_write INTEGER NOT NULL DEFAULT 0;

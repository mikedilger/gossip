
CREATE TABLE person_new (
    pubkey TEXT PRIMARY KEY NOT NULL,
    petname TEXT DEFAULT NULL,
    followed INTEGER NOT NULL DEFAULT 0,
    followed_last_updated INTEGER NOT NULL DEFAULT 0,
    muted INTEGER NOT NULL DEFAULT 0,
    metadata TEXT DEFAULT NULL,
    metadata_created_at INTEGER DEFAULT NULL,
    metadata_last_received INTEGER NOT NULL DEFAULT 0,
    nip05_valid INTEGER NOT NULL DEFAULT 0,
    nip05_last_checked INTEGER DEFAULT NULL,
    relay_list_created_at INTEGER DEFAULT NULL,
    relay_list_last_received INTEGER NOT NULL DEFAULT 0
) WITHOUT ROWID;

INSERT INTO person_new (
  pubkey, petname, followed, followed_last_updated, muted,
  metadata, metadata_created_at, metadata_last_received,
  nip05_valid, nip05_last_checked,
  relay_list_created_at, relay_list_last_received
) SELECT
  pubkey, petname, IFNULL(followed,0), followed_last_updated, IFNULL(muted,0),
  metadata, IFNULL(metadata_at,0), 0,
  IFNULL(nip05_valid,0), nip05_last_checked,
  relay_list_created_at, relay_list_last_received
FROM person;

-- SQLite does not update constraints when you rename tables.
-- so we have to do this manually.
CREATE TABLE IF NOT EXISTS "person_relay_new" (
    person TEXT NOT NULL,
    relay TEXT NOT NULL,
    last_fetched INTEGER DEFAULT NULL,
    last_suggested_kind3 INTEGER DEFAULT NULL,
    last_suggested_nip05 INTEGER DEFAULT NULL,
    last_suggested_bytag INTEGER DEFAULT NULL,
    read INTEGER NOT NULL DEFAULT 0,
    write INTEGER NOT NULL DEFAULT 0,
    manually_paired_read INTEGER NOT NULL DEFAULT 0,
    manually_paired_write INTEGER NOT NULL DEFAULT 0,
    UNIQUE(person, relay),
    CONSTRAINT person_relay_fk_person FOREIGN KEY (person) REFERENCES "person_new" (pubkey) ON DELETE CASCADE,
    CONSTRAINT person_relay_fk_relay FOREIGN KEY (relay) REFERENCES "relay" (url) ON DELETE CASCADE
);
INSERT INTO person_relay_new
  SELECT person, relay, last_fetched, last_suggested_kind3, last_suggested_nip05,
         last_suggested_bytag, read, write, manually_paired_read, manually_paired_write
  FROM person_relay;

UPDATE person_new SET relay_list_created_at=NULL WHERE relay_list_created_at=0;

ALTER TABLE person RENAME TO person_old;
ALTER TABLE person_relay RENAME TO person_relay_old;

ALTER TABLE person_new RENAME TO person;
ALTER TABLE person_relay_new RENAME TO person_relay;

DROP TABLE person_old;
DROP TABLE person_relay_old;

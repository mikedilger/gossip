CREATE TABLE relay_new (
    url TEXT PRIMARY KEY NOT NULL,
    success_count INTEGER NOT NULL DEFAULT 0,
    failure_count INTEGER NOT NULL DEFAULT 0,
    last_connected_at INTEGER DEFAULT NULL,
    last_general_eose_at INTEGER DEFAULT NULL,
    rank INTEGER NOT NULL DEFAULT 3,
    hidden INTEGER NOT NULL DEFAULT 0,
    usage_bits INTEGER NOT NULL DEFAULT 0
) WITHOUT ROWID;

INSERT INTO relay_new (
  url, success_count, failure_count, last_connected_at, last_general_eose_at,
  rank, hidden, usage_bits
) SELECT
  url, success_count, failure_count, last_connected_at, last_general_eose_at,
  rank, hidden, ((read * 1) + (write * 2) + (advertise * 4) + (read * 8) + (write * 16) + (read * 32))
  FROM relay;

-- We have to rebuild person_relay since its foreign key now needs to point to relay_new
CREATE TABLE person_relay_new (
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
    CONSTRAINT person_relay_fk_person FOREIGN KEY (person) REFERENCES person (pubkey) ON DELETE CASCADE,
    CONSTRAINT person_relay_fk_relay FOREIGN KEY (relay) REFERENCES relay_new (url) ON DELETE CASCADE
);
INSERT INTO person_relay_new (
  person, relay, last_fetched, last_suggested_kind3,
  last_suggested_nip05, last_suggested_bytag, read, write,
  manually_paired_read, manually_paired_write
) SELECT
  person, relay, last_fetched, last_suggested_kind3,
  last_suggested_nip05, last_suggested_bytag, read, write,
  manually_paired_read, manually_paired_write
  FROM person_relay;


-- We have to rebuild event_relay since its foreign key now needs to point to relay_new
CREATE TABLE event_relay_new (
    event TEXT NOT NULL,
    relay TEXT NOT NULL,
    when_seen INTEGER NOT NULL,
    UNIQUE (event, relay),
    CONSTRAINT event_relay_fk_event FOREIGN KEY (event) REFERENCES event (id) ON DELETE CASCADE,
    CONSTRAINT event_relay_fk_relay FOREIGN KEY (relay) REFERENCES relay_new (url) ON DELETE CASCADE
);
INSERT INTO event_relay_new (event, relay, when_seen) SELECT event, relay, when_seen FROM event_relay;


-- Now lets replace the tables
ALTER TABLE person_relay RENAME TO person_relay_old;
ALTER TABLE person_relay_new RENAME TO person_relay;

ALTER TABLE event_relay RENAME TO event_relay_old;
ALTER TABLE event_relay_new RENAME TO event_relay;

ALTER TABLE relay RENAME TO relay_old;
ALTER TABLE relay_new RENAME TO relay;

-- And drop the old ones
DROP TABLE person_relay_old;
DROP TABLE event_relay_old;
DROP TABLE relay_old;






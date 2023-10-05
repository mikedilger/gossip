INSERT INTO person (pubkey) SELECT distinct person FROM person_relay WHERE person NOT IN (select pubkey from person);

ALTER TABLE person_relay RENAME TO old_person_relay;
CREATE TABLE person_relay (
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
    CONSTRAINT person_relay_fk_relay FOREIGN KEY (relay) REFERENCES relay (url) ON DELETE CASCADE
);
INSERT OR IGNORE INTO person_relay (person, relay, last_fetched, last_suggested_kind3, last_suggested_nip05,
                          last_suggested_bytag, read, write, manually_paired_read, manually_paired_write)
  SELECT person, relay, last_fetched, last_suggested_kind3, last_suggested_nip05,
                          last_suggested_bytag, read, write, manually_paired_read, manually_paired_write
  FROM old_person_relay;
DROP TABLE old_person_relay;

DELETE FROM event_relay WHERE event NOT IN (select id from event);

ALTER TABLE event_relay RENAME TO old_event_relay;
CREATE TABLE event_relay (
    event TEXT NOT NULL,
    relay TEXT NOT NULL,
    when_seen INTEGER NOT NULL,
    UNIQUE (event, relay),
    CONSTRAINT event_relay_fk_event FOREIGN KEY (event) REFERENCES event (id) ON DELETE CASCADE,
    CONSTRAINT event_relay_fk_relay FOREIGN KEY (relay) REFERENCES relay (url) ON DELETE CASCADE
);
INSERT OR IGNORE INTO event_relay (event, relay, when_seen)
  SELECT event, relay, when_seen FROM old_event_relay;
DROP TABLE old_event_relay;

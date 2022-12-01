CREATE TABLE settings (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
) WITHOUT ROWID;

CREATE TABLE person (
    pubkey TEXT PRIMARY KEY NOT NULL,
    name TEXT DEFAULT NULL,
    about TEXT DEFAULT NULL,
    picture TEXT DEFAULT NULL,
    dns_id TEXT DEFAULT NULL,
    dns_id_valid INTEGER DEFAULT 0,
    dns_id_last_checked INTEGER DEFAULT NULL,
    followed INTEGER DEFAULT 0
) WITHOUT ROWID;

CREATE TABLE relay (
    url TEXT PRIMARY KEY NOT NULL,
    success_count INTEGER NOT NULL DEFAULT 0,
    failure_count INTEGER NOT NULL DEFAULT 0,
    rank INTEGER DEFAULT 3
) WITHOUT ROWID;

CREATE TABLE person_relay (
    person TEXT NOT NULL,
    relay TEXT NOT NULL,
    recommended INTEGER DEFAULT 0,
    last_fetched INTEGER DEFAULT NULL,
    UNIQUE(person, relay)
);

CREATE TABLE contact (
    source TEXT NOT NULL,
    contact TEXT NOT NULL,
    relay TEXT DEFAULT NULL,
    petname TEXT DEFAULT NULL,
    UNIQUE(source, contact)
);

CREATE TABLE event (
    id TEXT PRIMARY KEY NOT NULL,
    raw TEXT NOT NULL,
    pubkey TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    kind INTEGER NOT NULL,
    content TEXT NOT NULL,
    ots TEXT DEFAULT NULL
) WITHOUT ROWID;

CREATE TABLE event_tag (
    event TEXT NOT NULL,
    seq INTEGER NOT NULL,
    label TEXT DEFAULT NULL,
    field0 TEXT DEFAULT NULL,
    field1 TEXT DEFAULT NULL,
    field2 TEXT DEFAULT NULL,
    field3 TEXT DEFAULT NULL,
    CONSTRAINT fk_event
      FOREIGN KEY (event) REFERENCES event (id)
      ON DELETE CASCADE
);

CREATE TABLE event_seen (
    event TEXT NOT NULL,
    relay TEXT NOT NULL,
    when_seen INTEGER NOT NULL,
    UNIQUE (event, relay),
    CONSTRAINT fk_event
      FOREIGN KEY (event) REFERENCES event (id)
      ON DELETE CASCADE
);

INSERT INTO settings (key, value) values ('version', '1');
INSERT INTO settings (key, value) values ('user_public_key', '');
INSERT INTO settings (key, value) values ('user_private_key', '');
INSERT INTO settings (key, value) values ('overlap', '600');
INSERT INTO settings (key, value) values ('feed_chunk', '43200');
INSERT INTO settings (key, value) values ('autofollow', '0');

INSERT OR IGNORE INTO relay (url) values
('wss://nostr-pub.wellorder.net'),
('wss://nostr.bitcoiner.social'),
('wss://nostr-relay.wlvs.space'),
('wss://nostr-pub.semisol.dev'),
('wss://relay.damus.io'),
('wss://nostr.openchain.fr'),
('wss://nostr.delo.software'),
('wss://relay.nostr.info'),
('wss://nostr.oxtr.dev'),
('wss://nostr.ono.re'),
('wss://relay.grunch.dev'),
('wss://nostr.sandwich.farm');

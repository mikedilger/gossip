
ALTER TABLE person RENAME TO person_old;

CREATE TABLE person (
    pubkey TEXT PRIMARY KEY NOT NULL,
    metadata TEXT DEFAULT NULL,
    metadata_at integer default null,
    nip05_valid INTEGER DEFAULT 0,
    nip05_last_checked INTEGER DEFAULT NULL,
    followed INTEGER DEFAULT 0,
    followed_last_updated INTEGER NOT NULL DEFAULT 0
) WITHOUT ROWID;


INSERT INTO person (
  pubkey,
  metadata, metadata_at,
  nip05_valid, nip05_last_checked,
  followed, followed_last_updated
)
SELECT
  pubkey,
  json_object('name', name, 'about', about, 'picture', picture, 'nip05', dns_id),
  metadata_at,
  dns_id_valid, dns_id_last_checked,
  followed, followed_last_updated
FROM person_old;

UPDATE person SET metadata=null WHERE metadata_at is null;

DROP TABLE person_old;

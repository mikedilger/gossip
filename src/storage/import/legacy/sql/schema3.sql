
ALTER TABLE person_relay ADD COLUMN last_suggested_kind2 INTEGER DEFAULT NULL;
ALTER TABLE person_relay ADD COLUMN last_suggested_kind3 INTEGER DEFAULT NULL;
ALTER TABLE person_relay ADD COLUMN last_suggested_nip23 INTEGER DEFAULT NULL;
ALTER TABLE person_relay ADD COLUMN last_suggested_nip35 INTEGER DEFAULT NULL;
ALTER TABLE person_relay ADD COLUMN last_suggested_bytag INTEGER DEFAULT NULL;
ALTER TABLE person_relay DROP COLUMN recommended;

CREATE TABLE event_relationship (
  original TEXT NOT NULL,
  referring TEXT NOT NULL,
  relationship TEXT CHECK (relationship IN ('reply', 'quote', 'reaction', 'deletion')) NOT NULL,
  content TEXT DEFAULT NULL,
  UNIQUE(original, referring)
);

CREATE TABLE event_hashtag (
  event TEXT NOT NULL,
  hashtag TEXT NOT NULL,
  UNIQUE(event, hashtag)
);

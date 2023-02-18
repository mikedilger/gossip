
ALTER TABLE event_hashtag RENAME TO event_hashtag_old;
CREATE TABLE event_hashtag (
  event TEXT NOT NULL,
  hashtag TEXT NOT NULL,
  UNIQUE(event, hashtag),
  CONSTRAINT fk_event FOREIGN KEY (event) REFERENCES event (id) ON DELETE CASCADE
);
INSERT OR IGNORE INTO event_hashtag (event, hashtag) SELECT event, hashtag FROM event_hashtag_old;
DROP TABLE event_hashtag_old;


ALTER TABLE event_relationship RENAME TO event_relationship_old;
CREATE TABLE event_relationship (
  original TEXT NOT NULL,
  refers_to TEXT NOT NULL,
  relationship TEXT CHECK (relationship IN ('reply', 'quote', 'reaction', 'deletion')) NOT NULL,
  content TEXT DEFAULT NULL,
  CONSTRAINT fk_original FOREIGN KEY (original) REFERENCES event (id) ON DELETE CASCADE,
  UNIQUE(original, refers_to)
);
INSERT OR IGNORE INTO event_relationship (original, refers_to, relationship, content)
  SELECT original, referring, relationship, content FROM event_relationship_old;
DROP TABLE event_relationship_old;

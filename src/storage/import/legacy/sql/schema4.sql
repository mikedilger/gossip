
ALTER TABLE relay ADD COLUMN last_success_at INTEGER DEFAULT NULL;

PRAGMA foreign_keys=off;
BEGIN TRANSACTION;
ALTER TABLE event_tag RENAME TO event_tag_old;
CREATE TABLE event_tag (
    event TEXT NOT NULL,
    seq INTEGER NOT NULL,
    label TEXT DEFAULT NULL,
    field0 TEXT DEFAULT NULL,
    field1 TEXT DEFAULT NULL,
    field2 TEXT DEFAULT NULL,
    field3 TEXT DEFAULT NULL,
    UNIQUE (event, seq),
    CONSTRAINT fk_event
      FOREIGN KEY (event) REFERENCES event (id)
      ON DELETE CASCADE
);
INSERT OR IGNORE INTO event_tag SELECT * FROM event_tag_old;
COMMIT;
PRAGMA foreign_keys=on;
DROP TABLE event_tag_old;

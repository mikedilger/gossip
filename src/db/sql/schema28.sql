CREATE TABLE event_flags (
  event TEXT PRIMARY KEY NOT NULL,
  viewed INTEGER NOT NULL DEFAULT 0,
  CONSTRAINT fk_event FOREIGN KEY (event) REFERENCES event (id) ON DELETE CASCADE
);

RENAME TABLE event_seen TO event_relay;

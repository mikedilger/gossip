CREATE TABLE event_seen (
    id TEXT NOT NULL,
    url TEXT NOT NULL,
    when_seen TEXT NOT NULL
);

ALTER TABLE person_contact RENAME TO contact;
ALTER TABLE contact RENAME COLUMN person TO source;
ALTER TABLE person RENAME COLUMN following TO are_following;

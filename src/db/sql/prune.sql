
PRAGMA foreign_keys = ON;

-- Delete stale kind=0,3 events (keep the last one)
DELETE FROM event WHERE kind IN (0,3) AND created_at != (SELECT max(created_at) FROM event as event_inner WHERE kind=event.kind AND pubkey=event.pubkey);

-- Delete overridden replaceable events
DELETE FROM event WHERE kind>=10000 and kind<20000 AND created_at != (SELECT max(created_at) FROM event as event_inner WHERE kind=event.kind AND pubkey=event.pubkey);

-- Delete all ephemeral events
DELETE FROM event WHERE kind>=20000 and kind<30000;

-- Delete kind=1 events older than 1 week
DELETE FROM event WHERE kind!=0 AND kind!=3 AND kind!=10001 AND created_at < strftime('%s', 'now') - 60*60*24*7;

-- Due to foreign keys and cascades, the following will also be cleaned up
--   event_relationship
--   event_hashtag
--   event_tag
--   event_seen

VACUUM;

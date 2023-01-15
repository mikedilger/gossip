
PRAGMA foreign_keys = ON;

-- Delete stale kind=0 events (keep the last one)
DELETE FROM event WHERE kind=0 AND created_at != (SELECT max(created_at) FROM event as event_inner WHERE kind=0 AND pubkey=event.pubkey);

-- Delete stale kind=3 events (keep the last one)
DELETE FROM event WHERE kind=3 AND created_at != (SELECT max(created_at) FROM event as event_inner WHERE kind=3 AND pubkey=event.pubkey);

-- Delete stale kind=10001 events (keep the last one)
DELETE FROM event WHERE kind=10001 AND created_at != (SELECT max(created_at) FROM event as event_inner WHERE kind=10001 AND pubkey=event.pubkey);

-- Delete kind=1 events older than 1 week
DELETE FROM event WHERE kind!=0 AND kind!=3 AND kind!=10001 AND created_at < strftime('%s', 'now') - 60*60*24*7;

-- Due to foreign keys and cascades, the following will also be cleaned up
--   event_relationship
--   event_hashtag
--   event_tag
--   event_seen

VACUUM;

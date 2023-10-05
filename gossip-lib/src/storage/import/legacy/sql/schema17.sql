
-- I tried to merge records with relays ending in '/' to records where they didn't, but SQLITE makes it very hard to update joins (you can't do it directly, and I think the join ON clause is too complex to use EXISTS tricks).
-- So I'm just going to delete such records.  The data lost is statistical and not critical.

DELETE FROM event_seen WHERE SUBSTR(relay, LENGTH(relay), 1) == '/';
DELETE FROM person_relay WHERE SUBSTR(relay, LENGTH(relay), 1) == '/';
DELETE FROM relay WHERE SUBSTR(url, LENGTH(url), 1) == '/';

DELETE FROM event_seen WHERE SUBSTR(relay, 0, 15) == 'wss://127.0.0.1';
DELETE FROM person_relay WHERE SUBSTR(relay, 0, 15) == 'wss://127.0.0.1';
DELETE FROM relay WHERE SUBSTR(url, 0, 15) == 'wss://127.0.0.1';

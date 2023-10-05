
-- Again this is a PITA to merge these.
-- So I'm just going to delete such records
-- This data is mostly just statistical (but we might lose some person-relay pairs)

DELETE FROM relay WHERE url LIKE ' %';
DELETE FROM relay WHERE url LIKE '% ';
DELETE FROM relay WHERE url LIKE '	%';
DELETE FROM relay WHERE url LIKE '%	';
DELETE FROM person_relay WHERE relay LIKE ' %';
DELETE FROM person_relay WHERE relay LIKE '% ';
DELETE FROM person_relay WHERE relay LIKE '	%';
DELETE FROM person_relay WHERE relay LIKE '%	';
DELETE FROM event_seen WHERE relay LIKE ' %';
DELETE FROM event_seen WHERE relay LIKE '% ';
DELETE FROM event_seen WHERE relay LIKE '	%';
DELETE FROM event_seen WHERE relay LIKE '%	';

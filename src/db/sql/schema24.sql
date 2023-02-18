-- unused setting
DELETE FROM settings WHERE key='autofollow';
DELETE FROM settings WHERE key='thread_view_ancestors';
DELETE FROM settings WHERE key='thread_view_replies';
DELETE FROM settings WHERE key='view_posts_referred_to';
DELETE FROM settings WHERE key='view_posts_referring_to';
DELETE FROM settings WHERE key='view_threaded';

-- unused table
DROP TABLE contact;

-- missing data
ALTER TABLE person ADD COLUMN petname TEXT DEFAULT NULL;

-- move local settings
CREATE TABLE local_settings (
  schema_version INTEGER DEFAULT 0,
  encrypted_private_key TEXT DEFAULT NULL
);

INSERT INTO local_settings (schema_version, encrypted_private_key)
SELECT a.value, b.value FROM settings a INNER JOIN settings b
  ON a.key='version' AND b.key='encrypted_private_key';

DELETE FROM settings WHERE key='version';
DELETE FROM settings WHERE key='encrypted_private_key';

ALTER TABLE local_settings ADD COLUMN last_contact_list_edit INTEGER NOT NULL DEFAULT 0;

-- Initial value (may not be correct but we don't have the actual value)
UPDATE local_settings SET last_contact_list_edit=(SELECT created_at FROM event WHERE pubkey=(SELECT value FROM settings WHERE key='public_key') AND kind=3 ORDER BY created_at DESC LIMIT 1);

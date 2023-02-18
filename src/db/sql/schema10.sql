ALTER TABLE relay RENAME COLUMN last_success_at TO last_connected_at;
ALTER TABLE relay ADD COLUMN last_general_eose_at INTEGER DEFAULT NULL;

-- Start at last_connected_at
UPDATE relay SET last_general_eose_at = last_connected_at;

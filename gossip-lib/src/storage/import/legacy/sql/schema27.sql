-- New indices for event_tag
-- The sub-query in build_tag_condition filters by label and field0
CREATE INDEX event_tag_lf0 ON event_tag(label, field0);
-- populate_new_relays filters by label and field1
CREATE INDEX event_tag_lf1 ON event_tag(label, field1);
-- The LEFT JOIN in fetch_reply_related filters by event, label and field0
CREATE INDEX event_tag_elf0 ON event_tag(event, label, field0);

-- New indices for event
-- fetch_last_contact_list filters by kind and pubkey, then sorts by created_at
CREATE INDEX event_pkc ON event(pubkey, kind, created_at);
-- The LEFT JOIN in fetch_reply_related filters by id, kind and created_at
CREATE UNIQUE INDEX event_ikc ON event(id, kind, created_at);

-- Reset every person.metadata_at so it will be reloaded (probably from local events)
-- with code that now handles the additional fields
update person set metadata_at = null;

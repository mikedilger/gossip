
use crate::Error;
use crate::db::DbPerson;
use nostr_proto::{Metadata, PublicKeyHex, Unixtime};
use std::collections::HashMap;

// A synchronized person.
pub struct SPerson {
    pub person: DbPerson,
    pub db_stale: bool,
    pub js_stale: bool,
}

impl SPerson {
    pub fn new_from_db(person: DbPerson) -> SPerson {
        SPerson {
            person,
            db_stale: false,
            js_stale: true,
        }
    }

    pub fn new_from_event(person: DbPerson) -> SPerson {
        SPerson {
            person,
            db_stale: true,
            js_stale: true,
        }
    }
}

// The PersonSynchro keeps a list of person data and helps
// to keep it synchronized with both the database and with
// javascript
pub struct PersonSynchro {
    pub people: HashMap<PublicKeyHex, SPerson>
}

impl PersonSynchro {
    pub fn new() -> PersonSynchro {
        PersonSynchro {
            people: HashMap::new()
        }
    }

    pub(super) async fn load_all_from_database(&mut self) -> Result<(), Error> {
        let mut dbpeople = DbPerson::fetch(None).await?;
        for dbperson in dbpeople.drain(..) {
            let pubkey = dbperson.pubkey.clone();
            let sperson = SPerson::new_from_db(dbperson);
            self.people.insert(pubkey, sperson);
        }

        Ok(())
    }

    pub(super) fn update_from_event(&mut self,
                                    pubkey: PublicKeyHex,
                                    created_at: Unixtime,
                                    metadata: Metadata)
    {
        // Get (or create) the person in our HashMap
        let sperson_ref = self.people.entry(pubkey.clone()).or_insert(
            SPerson::new_from_event(
                DbPerson::new(pubkey.clone())
            )
        );

        // Do not update the metadata if ours is newer
        if let Some(metadata_at) = sperson_ref.person.metadata_at {
            if created_at.0 <= metadata_at {
                // Old metadata. Ignore it
                return;
            }
        }

        // Update the metadata
        sperson_ref.person.name = metadata.name;
        sperson_ref.person.about = metadata.about;
        sperson_ref.person.picture = metadata.picture;
        if sperson_ref.person.dns_id != metadata.nip05 {
            sperson_ref.person.dns_id = metadata.nip05;
            sperson_ref.person.dns_id_valid = 0; // changed, so reset to invalid
            sperson_ref.person.dns_id_last_checked = None; // we haven't checked this one yet
        }
        sperson_ref.person.metadata_at = Some(created_at.0);
    }

    pub(super) async fn sync_to_database(&mut self) -> Result<(), Error> {
        for (_,sperson) in self.people.iter_mut().filter(|(_,p)| p.db_stale) {
            DbPerson::update(sperson.person.clone()).await?;
            sperson.db_stale = false;
        }

        Ok(())
    }

    // This returns all the people for synchronizing to javascript.  It presumes the
    // caller (Overlord) will send them.
    pub(super) async fn for_sync_to_javascript(&mut self) -> Result<Vec<DbPerson>, Error> {
        let mut people: Vec<DbPerson> = Vec::new();

        for (_,sperson) in self.people.iter_mut().filter(|(_,p)| p.js_stale) {
            people.push(sperson.person.clone());
            sperson.js_stale = false;
        }

        Ok(people)
    }

    pub(super) fn followed_pubkeys(&self) -> Vec<PublicKeyHex> {
        self.people.iter()
            .map(|(_,p)| p)
            .filter(|p| p.person.followed==1)
            .map(|p| p.person.pubkey.clone())
            .collect()
    }
}

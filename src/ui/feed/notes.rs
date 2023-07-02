use super::notedata::NoteData;
use crate::globals::GLOBALS;
use nostr_types::{Id, PublicKeyHex};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

/// a 'note' is a processed event
pub struct Notes {
    notes: HashMap<Id, Rc<RefCell<NoteData>>>,
}

impl Notes {
    pub fn new() -> Notes {
        Notes {
            notes: HashMap::new(),
        }
    }

    /*
    /// Drop NoteData objects that do not have a
    /// correlated event in the event cache
    pub(super) fn cache_invalidate_missing_events(&mut self) {
        self.notes.retain(|id,_| GLOBALS.events.contains_key(id));
    }
     */

    /// Drop NoteData for a specific note
    pub(super) fn cache_invalidate_note(&mut self, id: &Id) {
        self.notes.remove(id);
    }

    /// Drop all NoteData for a given person
    pub(in crate::ui) fn cache_invalidate_person(&mut self, pubkey: &PublicKeyHex) {
        self.notes
            .retain(|_, note| note.borrow().author.pubkey != *pubkey);
    }

    pub(super) fn try_update_and_get(&mut self, id: &Id) -> Option<Rc<RefCell<NoteData>>> {
        if self.notes.contains_key(id) {
            // get a mutable reference to update reactions, then give it back
            if let Some(pair) = self.notes.get(id) {
                if let Ok(mut mut_ref) = pair.try_borrow_mut() {
                    mut_ref.update_reactions();
                }
            }
            // return from cache
            return self._try_get_and_borrow(id);
        } else {
            // otherwise try to create new and add to cache
            if let Some(event) = GLOBALS.events.get(id) {
                let note = NoteData::new(event);
                // add to cache
                let ref_note = Rc::new(RefCell::new(note));
                self.notes.insert(*id, ref_note);
                return self._try_get_and_borrow(id);
            } else {
                // send a worker to try and load it from the database
                // if it's in the db it will go into the cache and be
                // available on a future UI update
                let id_copy = id.to_owned();
                tokio::spawn(async move {
                    if let Err(e) = GLOBALS.events.get_local(id_copy).await {
                        tracing::error!("{}", e);
                    }
                });
            }
        }

        None
    }

    /*
    pub(super) fn try_get(&mut self, id: &Id) -> Option<Rc<RefCell<NoteData>>> {
        if self.notes.contains_key(id) {
            // return from cache
            return self._try_get_and_borrow(id)
        } else {
            // otherwise try to create new and add to cache
            if let Some(event) = GLOBALS.events.get(id) {
                if let Some(note) = NoteData::new(event) {
                    // add to cache
                    let ref_note = Rc::new(RefCell::new(note));
                    self.notes.insert(*id, ref_note);
                    return self._try_get_and_borrow(id);
                }
            } else {
                // send a worker to try and load it from the database
                // if it's in the db it will go into the cache and be
                // available on the next UI update
                let id_copy = id.to_owned();
                tokio::spawn(async move {
                    if let Err(e) = GLOBALS.events.get_local(id_copy).await {
                        tracing::error!("{}", e);
                    }
                });
            }
        }
        None
    }
     */

    fn _try_get_and_borrow(&self, id: &Id) -> Option<Rc<RefCell<NoteData>>> {
        if let Some(value) = self.notes.get(id) {
            return Some(value.clone());
        }
        None
    }
}

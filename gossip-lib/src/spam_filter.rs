use crate::globals::GLOBALS;
use crate::people::PersonList;
use crate::profile::Profile;
use crate::storage::{PersonTable, Table};
use nostr_types::{Event, EventKind, Id, PublicKey};
use rhai::{Engine, Scope, AST};
use std::fs;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EventFilterAction {
    Deny,
    Allow,
    MuteAuthor,
}

pub fn load_script(engine: &Engine) -> Option<AST> {
    let mut path = match Profile::profile_dir() {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("Profile failed: {}", e);
            return None;
        }
    };

    path.push("filter.rhai");

    let script = match fs::read_to_string(&path) {
        Ok(script) => script,
        Err(e) => {
            tracing::info!("No spam filter: {}", e);
            return None;
        }
    };

    let ast = match engine.compile(script) {
        Ok(ast) => ast,
        Err(e) => {
            tracing::error!("Failed to compile spam filter: {}", e);
            return None;
        }
    };

    tracing::info!("Spam filter loaded.");

    Some(ast)
}

pub fn filter_event(event: Event) -> EventFilterAction {
    if GLOBALS.spam_filter.is_none() {
        EventFilterAction::Allow
    } else if event.kind == EventKind::GiftWrap {
        if let Ok(rumor) = GLOBALS.identity.unwrap_giftwrap(&event) {
            // id from giftwrap, the rest from rumor
            inner_filter(event.id, rumor.pubkey, rumor.kind, rumor.content)
        } else {
            EventFilterAction::Allow
        }
    } else {
        inner_filter(event.id, event.pubkey, event.kind, event.content)
    }
}

fn inner_filter(id: Id, pubkey: PublicKey, kind: EventKind, content: String) -> EventFilterAction {
    // Only apply to feed-displayable events
    if !kind.is_feed_displayable() {
        return EventFilterAction::Allow;
    }

    let author = match PersonTable::read_record(pubkey, None) {
        Ok(a) => a,
        Err(_) => None,
    };

    // Do not apply to people you follow
    if GLOBALS
        .people
        .is_person_in_list(&pubkey, PersonList::Followed)
    {
        return EventFilterAction::Allow;
    }

    let mut scope = Scope::new();
    scope.push("id", id.as_hex_string());
    scope.push("pubkey", pubkey.as_hex_string());
    scope.push("kind", <EventKind as Into<u32>>::into(kind));
    // TBD: tags
    scope.push("content", content);
    scope.push(
        "nip05valid",
        match &author {
            Some(a) => a.nip05_valid,
            None => false,
        },
    );
    scope.push(
        "name",
        match &author {
            Some(p) => p.best_name(),
            None => "".to_owned(),
        },
    );

    filter_with_script(scope)
}

fn filter_with_script(mut scope: Scope) -> EventFilterAction {
    let ast = match &GLOBALS.spam_filter {
        Some(ast) => ast,
        None => return EventFilterAction::Allow,
    };

    match GLOBALS
        .spam_filter_engine
        .call_fn::<i64>(&mut scope, ast, "filter", ())
    {
        Ok(action) => match action {
            0 => EventFilterAction::Deny,
            1 => EventFilterAction::Allow,
            2 => EventFilterAction::MuteAuthor,
            _ => EventFilterAction::Allow,
        },
        Err(ear) => {
            tracing::error!("{}", ear);
            EventFilterAction::Allow
        }
    }
}

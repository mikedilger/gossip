use crate::globals::GLOBALS;
use crate::people::Person;
use crate::profile::Profile;
use nostr_types::{Event, EventKind, Id, Rumor};
use rhai::{Engine, Scope, AST};
use std::fs;

#[derive(Clone, Copy, Debug)]
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

pub fn filter_rumor(rumor: Rumor, author: Option<Person>, id: Id) -> EventFilterAction {
    if GLOBALS.filter.is_none() {
        return EventFilterAction::Allow;
    }

    let mut scope = Scope::new();

    scope.push("id", id.as_hex_string()); // ID of the gift wrap
    scope.push("pubkey", rumor.pubkey.as_hex_string());
    scope.push("kind", <EventKind as Into<u32>>::into(rumor.kind));
    // FIXME tags
    scope.push("content", rumor.content.clone());
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
            None => "".to_owned()
        }
    );

    filter(scope, id)
}

pub fn filter_event(event: Event, author: Option<Person>) -> EventFilterAction {
    if GLOBALS.filter.is_none() {
        return EventFilterAction::Allow;
    }

    let mut scope = Scope::new();

    scope.push("id", event.id.as_hex_string());
    scope.push("pubkey", event.pubkey.as_hex_string());
    scope.push("kind", <EventKind as Into<u32>>::into(event.kind));
    // FIXME tags
    scope.push("content", event.content.clone());
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
            None => "".to_owned()
        }
    );

    filter(scope, event.id)
}

fn filter(mut scope: Scope, id: Id) -> EventFilterAction {
    let ast = match &GLOBALS.filter {
        Some(ast) => ast,
        None => return EventFilterAction::Allow,
    };

    match GLOBALS
        .filter_engine
        .call_fn::<i64>(&mut scope, ast, "filter", ())
    {
        Ok(action) => match action {
            0 => {
                tracing::info!("SPAM FILTER BLOCKING EVENT {}", id.as_hex_string());
                EventFilterAction::Deny
            }
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

// Only call the filter if the author isn't followed

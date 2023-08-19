use crate::globals::GLOBALS;
use crate::people::Person;
use crate::profile::Profile;
use nostr_types::{Event, EventKind};
use rhai::{Engine, Scope, AST};
use std::fs;

#[derive(Clone, Copy, Debug)]
pub enum EventFilterAction {
    Deny,
    Allow,
    MuteAuthor,
}

pub fn load_script(engine: &Engine) -> Option<AST> {
    let profile = match Profile::current() {
        Ok(profile) => profile,
        Err(e) => {
            tracing::error!("Profile failed: {}", e);
            return None;
        }
    };

    let mut path = profile.profile_dir.clone();
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

pub fn filter(event: Event, author: Option<Person>) -> EventFilterAction {
    let ast = match &GLOBALS.filter {
        Some(ast) => ast,
        None => return EventFilterAction::Allow,
    };

    let mut scope = Scope::new();
    scope.push("id", event.id.as_hex_string());
    scope.push("pubkey", event.pubkey.as_hex_string());
    scope.push("kind", <EventKind as Into<u32>>::into(event.kind));
    // FIXME tags
    scope.push("content", event.content.clone());
    scope.push(
        "nip05valid",
        match author {
            Some(a) => a.nip05_valid,
            None => false,
        },
    );

    match GLOBALS
        .filter_engine
        .call_fn::<i64>(&mut scope, &ast, "filter", ())
    {
        Ok(action) => match action {
            0 => {
                tracing::info!("SPAM FILTER BLOCKING EVENT {}", event.id.as_hex_string());
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

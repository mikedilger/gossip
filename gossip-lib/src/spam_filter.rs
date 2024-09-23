use crate::globals::GLOBALS;
use crate::people::PersonList;
use crate::profile::Profile;
use crate::storage::{PersonTable, Table};
use nostr_types::{Event, EventKind, Id, PublicKey, Tag, Unixtime};
use rhai::{CallFnOptions, Engine, Scope, AST};
use std::fs;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EventFilterAction {
    Deny,
    Allow,
    MuteAuthor,
}

#[derive(Debug)]
pub enum EventFilterCaller {
    Process,
    Thread,
    Inbox,
    Global,
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

pub fn filter_event(event: Event, caller: EventFilterCaller, spamsafe: bool) -> EventFilterAction {
    // these are the same whether in giftwrap or noto
    let id = event.id;
    let pow = event.pow();

    if GLOBALS.spam_filter.is_none() {
        EventFilterAction::Allow
    } else if event.kind == EventKind::GiftWrap {
        if let Ok(rumor) = GLOBALS.identity.unwrap_giftwrap(&event) {
            // id from giftwrap, the rest from rumor
            inner_filter(
                id,
                rumor.pubkey,
                rumor.kind,
                rumor.content,
                rumor.tags,
                pow,
                caller,
                spamsafe,
            )
        } else {
            EventFilterAction::Allow
        }
    } else {
        inner_filter(
            id,
            event.pubkey,
            event.kind,
            event.content,
            event.tags,
            pow,
            caller,
            spamsafe,
        )
    }
}

fn inner_filter(
    id: Id,
    pubkey: PublicKey,
    kind: EventKind,
    content: String,
    mut tags: Vec<Tag>,
    pow: u8,
    caller: EventFilterCaller,
    spamsafe: bool,
) -> EventFilterAction {
    // Only apply to feed-displayable events
    if !kind.is_feed_displayable() {
        return EventFilterAction::Allow;
    }

    let author = match PersonTable::read_record(pubkey, None) {
        Ok(a) => a,
        Err(_) => None,
    };

    let muted = GLOBALS.people.is_person_in_list(&pubkey, PersonList::Muted);

    // Do not apply to people you follow
    if GLOBALS
        .people
        .is_person_in_list(&pubkey, PersonList::Followed)
    {
        return EventFilterAction::Allow;
    }

    let tags: Vec<Vec<String>> = tags.drain(..).map(|t| t.into_inner()).collect();

    // NOTE numbers in rhai are i64 or f32
    let mut scope = Scope::new();
    scope
        .push_constant("id", id.as_hex_string())
        .push_constant("pubkey", pubkey.as_hex_string())
        .push_constant("kind", <EventKind as Into<u32>>::into(kind) as i64)
        .push_constant("content", content)
        .push_constant("tags", tags)
        .push_constant("fof", GLOBALS.db().read_fof(pubkey).unwrap_or(0) as i64)
        // for backwards compatibility:
        .push_constant("wot", GLOBALS.db().read_fof(pubkey).unwrap_or(0) as i64)
        .push_constant(
            "nip05valid",
            match &author {
                Some(a) => a.nip05_valid,
                None => false,
            },
        )
        .push_constant(
            "name",
            match &author {
                Some(p) => p.best_name(),
                None => "".to_owned(),
            },
        )
        .push_constant("caller", format!("{:?}", caller))
        .push_constant(
            "seconds_known",
            match &author {
                Some(a) => Unixtime::now().0 - a.first_encountered,
                None => 0_i64,
            },
        )
        .push_constant("pow", pow as i64)
        .push_constant("spamsafe", spamsafe)
        .push_constant("muted", muted)
        .push_constant("DENY", 0_i64)
        .push_constant("ALLOW", 1_i64)
        .push_constant("MUTE", 2_i64);

    filter_with_script(scope)
}

fn filter_with_script(mut scope: Scope) -> EventFilterAction {
    // Get the pre-computed AST
    let ast = match &GLOBALS.spam_filter {
        Some(ast) => ast,
        None => return EventFilterAction::Allow,
    };

    // Do not bother to evaluate the AST, there are no imports
    let options = CallFnOptions::new().eval_ast(false);

    match GLOBALS.spam_filter_engine.call_fn_with_options::<i64>(
        options,
        &mut scope,
        ast,
        "filter",
        (),
    ) {
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

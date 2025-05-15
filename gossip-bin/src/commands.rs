use gossip_lib::{Error, ErrorKind, PersonList, PersonListMetadata, PersonTable, Table, GLOBALS};
use nostr_types::{
    EncryptedPrivateKey, Event, EventKind, Filter, Id, NAddr, NostrBech32, NostrUrl, ParsedTag,
    PreEvent, PrivateKey, PublicKey, RelayUrl, Tag, UncheckedUrl, Unixtime,
};
use std::collections::HashSet;
use std::env;
use std::time::Duration;
use zeroize::Zeroize;

#[derive(Debug, Clone)]
pub struct Command {
    cmd: &'static str,
    usage_params: &'static str,
    desc: &'static str,
}

impl Command {
    fn usage(&self, msg: String) -> Result<(), Error> {
        Err(ErrorKind::Usage(
            msg,
            format!("Usage: gossip {} {}", self.cmd, self.usage_params),
        )
        .into())
    }
}

const COMMANDS: [Command; 52] = [
    Command {
        cmd: "oneshot",
        usage_params: "{depends}",
        desc: "(this is for developer use only, please ignore it)",
    },
    Command {
        cmd: "add_person_list",
        usage_params: "<listname>",
        desc: "add a new person list with the given name",
    },
    Command {
        cmd: "backdate_eose",
        usage_params: "",
        desc: "backdate last_general_eose_at by 24 hours for every relay. This will usually cause gossip to refetch recent things.",
    },
    Command {
        cmd: "bech32_decode",
        usage_params: "<bech32string>",
        desc: "decode the bech32 string.",
    },
    Command {
        cmd: "bech32_encode_naddr",
        usage_params: "<kind> <pubkey> <d> [<relayurl>, ...]",
        desc: "encode an event address (parameterized replaceable event link).",
    },
    Command {
        cmd: "clear_timeouts",
        usage_params: "",
        desc: "clear relay avoidance timeouts.",
    },
    Command {
        cmd: "decrypt",
        usage_params: "<pubkey> <ciphertext>",
        desc: "decrypt the ciphertext from the pubkeyhex.",
    },
    Command {
        cmd: "delete_by_kind",
        usage_params: "<kind>",
        desc: "deletes all events of the given kind (be careful!)",
    },
    Command {
        cmd: "delete_by_id",
        usage_params: "<id>",
        desc: "deletes event of the given id",
    },
    Command {
        cmd: "delete_spam_by_content",
        usage_params: "<kind> <unixtime_since> <substring>",
        desc: "delete all feed-displayable events with content matching the substring (including giftwraps)",
    },
    Command {
        cmd: "delete_relay",
        usage_params: "<relayurl>",
        desc: "delete a relay record from storage. Be aware any event referencing it will cause it to be recreated.",
    },
    Command {
        cmd: "dpi",
        usage_params: "<dpi>",
        desc: "override the DPI setting",
    },
    Command {
        cmd: "disable_relay",
        usage_params: "<relayurl>",
        desc: "Set a relay rank to 0 so it will never connect, and also hide form thie list. This is better than delete because deleted relays quickly come back with default settings."
    },
    Command {
        cmd: "dump_handlers",
        usage_params: "",
        desc: "print all the web-based event handlers",
    },
    Command {
        cmd: "events_of_kind",
        usage_params: "<kind>",
        desc: "print IDs of all events of kind=<kind>",
    },
    Command {
        cmd: "events_of_pubkey",
        usage_params: "<pubkey>",
        desc: "print IDs of all events from <pubkeyhex>",
    },
    Command {
        cmd: "events_of_pubkey_and_kind",
        usage_params: "<pubkey> <kind>",
        desc: "print IDs of all events from <pubkeyhex> of kind=<kind>",
    },
    Command {
        cmd: "export_encrypted_key",
        usage_params: "",
        desc: "Export the encrypted private key",
    },
    Command {
        cmd: "force_migration_level",
        usage_params: "<level>",
        desc: "DANGEROUS! force storage migration level",
    },
    Command {
        cmd: "giftwraps",
        usage_params: "",
        desc: "Decrypt all of your giftwraps",
    },
    Command {
        cmd: "help",
        usage_params: "<command>",
        desc: "show documentation of <command> if <command is specified, otherwise documentation of all commands",
    },
    Command {
        cmd: "import_encrypted_private_key",
        usage_params: "<ncryptsec>",
        desc: "import encrypted private key",
    },
    Command {
        cmd: "import_event",
        usage_params: "<event_json>",
        desc: "import and process a JSON event",
    },
    Command {
        cmd: "import_lmdb_events",
        usage_params: "<other_lmdb_dir>",
        desc: "Import all events from a given LMDB directory and insert them into the live database",
    },
    Command {
        cmd: "keys",
        usage_params: "",
        desc: "Show keys (public and encrypted private)",
    },
    Command {
        cmd: "login",
        usage_params: "",
        desc: "login on the command line before starting the gossip GUI",
    },
    Command {
        cmd: "offline",
        usage_params: "",
        desc: "start gossip in offline mode",
    },
    Command {
        cmd: "print_event",
        usage_params: "<idhex>",
        desc: "print the event (in JSON) from the database that has the given id",
    },
    Command {
        cmd: "print_followed",
        usage_params: "",
        desc: "print every pubkey that is followed",
    },
    Command {
        cmd: "print_muted",
        usage_params: "",
        desc: "print every pubkey that is muted",
    },
    Command {
        cmd: "print_person_lists",
        usage_params: "",
        desc: "print every pubkey in every person list",
    },
    Command {
        cmd: "print_person",
        usage_params: "<pubkey>",
        desc: "print the given person",
    },
    Command {
        cmd: "print_person_relays",
        usage_params: "<pubkey>",
        desc: "print all the person-relay records for the given person",
    },
    Command {
        cmd: "print_relay",
        usage_params: "<url>",
        desc: "print the relay record",
    },
    Command {
        cmd: "print_relays",
        usage_params: "",
        desc: "print all the relay records",
    },
    Command {
        cmd: "print_seen_on",
        usage_params: "<idhex>",
        desc: "print the relays the event was seen on",
    },
    Command {
        cmd: "prune_cache",
        usage_params: "",
        desc: "prune old cached files",
    },
    Command {
        cmd: "prune_old_events",
        usage_params: "",
        desc: "prune old events (according to current prune settings)",
    },
    Command {
        cmd: "prune_unused_people",
        usage_params: "",
        desc: "prune unused people",
    },
    Command {
        cmd: "reaction_stats",
        usage_params: "",
        desc: "Show statistics on reactions",
    },
    Command {
        cmd: "rebuild_fof",
        usage_params: "",
        desc: "Rebuild friends-of-friends (will rebuild next time gossip starts)",
    },
    Command {
        cmd: "rebuild_indices",
        usage_params: "",
        desc: "Rebuild all event-related indices",
    },
    Command {
        cmd: "rename_person_list",
        usage_params: "<number> <newname>",
        desc: "Rename a person list",
    },
    Command {
        cmd: "reprocess_recent",
        usage_params: "",
        desc: "Reprocess events that came during the last 24 hours",
    },
    Command {
        cmd: "reprocess_relay_lists",
        usage_params: "",
        desc: "Reprocess relay lists (including kind 3 contents)",
    },
    Command {
        cmd: "reset_relay_auth",
        usage_params: "",
        desc: "Reset allow authentication settings on all relays (to unstated)",
    },
    Command {
        cmd: "reset_relay_connnect",
        usage_params: "",
        desc: "Reset allow connection settings on all relays (to unstated)",
    },
    Command {
        cmd: "theme",
        usage_params: "<dark | light>",
        desc: "Start gossip with the selected theme",
    },
    Command {
        cmd: "ungiftwrap",
        usage_params: "<idhex>",
        desc: "Unwrap the giftwrap event with the given ID and print the rumor (in JSON)",
    },
    Command {
        cmd: "verify",
        usage_params: "<idhex>",
        desc: "Verify if the given event signature is valid",
    },
    Command {
        cmd: "verify_json",
        usage_params: "<event_json>",
        desc: "Verify if the passed in event JSON's signature is valid",
    },
    Command {
        cmd: "wgpu_renderer",
        usage_params: "true | false",
        desc: "Enable/Disable the WGPU rendering backend",
    },
];

pub async fn handle_command(mut args: env::Args) -> Result<bool, Error> {
    let command_string = args.next().unwrap(); // must be there or we would not have been called

    let mut command: Option<Command> = None;
    for c in COMMANDS.iter() {
        if command_string == c.cmd {
            command = Some(c.to_owned());
            break;
        }
    }
    let command = match command {
        None => return Err(ErrorKind::UnknownCommand(command_string).into()),
        Some(c) => c,
    };

    match command.cmd {
        "oneshot" => oneshot(command, args)?,
        "add_person_list" => add_person_list(command, args)?,
        "backdate_eose" => backdate_eose()?,
        "bech32_decode" => bech32_decode(command, args)?,
        "bech32_encode_naddr" => bech32_encode_naddr(command, args)?,
        "clear_timeouts" => clear_timeouts()?,
        "decrypt" => decrypt(command, args).await?,
        "delete_by_kind" => delete_by_kind(command, args)?,
        "delete_by_id" => delete_by_id(command, args)?,
        "delete_spam_by_content" => delete_spam_by_content(command, args).await?,
        "delete_relay" => delete_relay(command, args)?,
        "dpi" => override_dpi(command, args)?,
        "disable_relay" => disable_relay(command, args)?,
        "dump_handlers" => dump_handlers()?,
        "events_of_kind" => events_of_kind(command, args)?,
        "events_of_pubkey" => events_of_pubkey(command, args)?,
        "events_of_pubkey_and_kind" => events_of_pubkey_and_kind(command, args)?,
        "export_encrypted_key" => export_encrypted_key()?,
        "force_migration_level" => force_migration_level(command, args)?,
        "giftwraps" => giftwraps(command).await?,
        "help" => help(command, args)?,
        "import_encrypted_private_key" => import_encrypted_private_key(command, args)?,
        "import_event" => import_event(command, args).await?,
        "keys" => keys()?,
        "login" => {
            login().await?;
            return Ok(false);
        }
        "import_lmdb_events" => import_lmdb_events(command, args).await?,
        "offline" => {
            offline()?;
            return Ok(false);
        }
        "print_event" => print_event(command, args)?,
        "print_followed" => print_followed(command)?,
        "print_muted" => print_muted(command)?,
        "print_person_lists" => print_person_lists(command)?,
        "print_person" => print_person(command, args)?,
        "print_person_relays" => print_person_relays(command, args)?,
        "print_relay" => print_relay(command, args)?,
        "print_relays" => print_relays(command)?,
        "print_seen_on" => print_seen_on(command, args)?,
        "prune_cache" => prune_cache()?,
        "prune_old_events" => prune_old_events()?,
        "prune_unused_people" => prune_unused_people()?,
        "reaction_stats" => reaction_stats(command, args)?,
        "rebuild_fof" => rebuild_fof()?,
        "rebuild_indices" => rebuild_indices().await?,
        "rename_person_list" => rename_person_list(command, args)?,
        "reprocess_recent" => reprocess_recent(command).await?,
        "reprocess_relay_lists" => reprocess_relay_lists()?,
        "reset_relay_auth" => reset_relay_auth()?,
        "reset_relay_connect" => reset_relay_connect()?,
        "theme" => {
            set_theme(command, args)?;
            return Ok(false);
        }
        "ungiftwrap" => ungiftwrap(command, args).await?,
        "verify" => verify(command, args)?,
        "verify_json" => verify_json(command, args)?,
        "wgpu_renderer" => wgpu_renderer(command, args)?,
        other => println!("Unknown command {}", other),
    }

    Ok(true)
}

pub fn help(_cmd: Command, mut args: env::Args) -> Result<(), Error> {
    if let Some(sub) = args.next() {
        for c in COMMANDS.iter() {
            if sub == c.cmd {
                println!("Usage:   gossip {} {}", c.cmd, c.usage_params);
                println!();
                println!("   {}", c.desc);
                return Ok(());
            }
        }
        println!("No such command {}", sub);
    } else {
        println!("Usage:   gossip [--rapid] <cmd>");
        println!();
        println!("  --rapid         Use faster storage access at the risk of data corruption");
        println!();
        println!("  <cmd> can be any of these:");
        for c in COMMANDS.iter() {
            if c.cmd == "oneshot" {
                continue;
            }
            println!("    {} {}", c.cmd, c.usage_params);
            println!("        {}", c.desc);
        }
    }
    Ok(())
}

pub fn oneshot(_cmd: Command, mut _args: env::Args) -> Result<(), Error> {
    // This code area is reserved for doing things that do not get committed
    Ok(())
}

pub fn add_person_list(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let listname = match args.next() {
        Some(s) => s,
        None => return cmd.usage("Missing listname parameter".to_string()),
    };

    let metadata = PersonListMetadata {
        dtag: listname.clone(),
        title: listname.clone(),
        ..Default::default()
    };

    let _list = GLOBALS.db().allocate_person_list(&metadata, None)?;
    Ok(())
}

pub fn backdate_eose() -> Result<(), Error> {
    let now = Unixtime::now();
    let ago = (now.0 - 60 * 60 * 24) as u64;

    GLOBALS.db().modify_all_relays(
        |relay| {
            if let Some(eose) = relay.last_general_eose_at {
                if eose > ago {
                    relay.last_general_eose_at = Some(ago);
                }
            }
        },
        None,
    )?;

    Ok(())
}

pub fn bech32_decode(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let mut param = match args.next() {
        Some(s) => s,
        None => return cmd.usage("Missing bech32string parameter".to_string()),
    };

    // Also work if prefixed with 'nostr:'
    if let Some(nurl) = NostrUrl::try_from_string(&param) {
        param = format!("{}", nurl.0);
    }

    if let Some(nb32) = NostrBech32::try_from_string(&param) {
        match nb32 {
            NostrBech32::CryptSec(epk) => {
                println!("Crypt Sec:");
                println!("  encrypted private key: {}", epk);
            }
            NostrBech32::NAddr(ea) => {
                println!("Event Address:");
                println!("  d={}", ea.d);
                println!(
                    "  relays={}",
                    ea.relays
                        .iter()
                        .map(|r| r.as_str().to_owned())
                        .collect::<Vec<String>>()
                        .join(", ")
                );
                println!("  kind={}", Into::<u32>::into(ea.kind));
                println!("  author={}", ea.author.as_hex_string());
            }
            NostrBech32::NEvent(ne) => {
                println!("NEvent:");
                println!("  id={}", ne.id.as_hex_string());
                println!(
                    "  relays={}",
                    ne.relays
                        .iter()
                        .map(|r| r.as_str().to_owned())
                        .collect::<Vec<String>>()
                        .join(", ")
                );
                if let Some(kind) = ne.kind {
                    println!("  kind={}", Into::<u32>::into(kind));
                }
                if let Some(author) = ne.author {
                    println!("  author={}", author.as_hex_string());
                }
            }
            NostrBech32::Id(id) => {
                println!("Id: {}", id.as_hex_string());
            }
            NostrBech32::Profile(profile) => {
                println!("Profile:");
                println!("  pubkey: {}", profile.pubkey.as_hex_string());
                println!(
                    "  relays={}",
                    profile
                        .relays
                        .iter()
                        .map(|r| r.as_str().to_owned())
                        .collect::<Vec<String>>()
                        .join(", ")
                );
            }
            NostrBech32::Pubkey(pubkey) => {
                println!("Pubkey: {}", pubkey.as_hex_string());
            }
            NostrBech32::Relay(url) => {
                println!("Relay URL: {}", url.0);
            }
        }
    } else if let Ok(mut key) = PrivateKey::try_from_bech32_string(&param) {
        println!("Private Key: {}", key.as_hex_string());
    } else {
        let data = bech32::decode(&param).unwrap();
        println!("DATA.0 = {}", data.0);
        println!("DATA.1 = {}", String::from_utf8_lossy(&data.1));
    }

    Ok(())
}

pub fn bech32_encode_naddr(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let kind: EventKind = match args.next() {
        Some(integer) => integer.parse::<u32>()?.into(),
        None => return cmd.usage("Missing kind parameter".to_string()),
    };

    let pubkey = match args.next() {
        Some(s) => match PublicKey::try_from_hex_string(&s, true) {
            Ok(pk) => pk,
            Err(_) => PublicKey::try_from_bech32_string(&s, true)?,
        },
        None => return cmd.usage("Missing pubkey parameter".to_string()),
    };

    let d = match args.next() {
        Some(d) => d,
        None => return cmd.usage("Missing d parameter".to_string()),
    };

    let mut urls: Vec<UncheckedUrl> = vec![];

    for s in args {
        urls.push(UncheckedUrl::from_string(s));
    }

    let ea = NAddr {
        d,
        relays: urls,
        kind,
        author: pubkey,
    };

    println!("{}", ea.as_bech32_string());

    Ok(())
}

pub fn clear_timeouts() -> Result<(), Error> {
    GLOBALS
        .db()
        .modify_all_relays(|r| r.avoid_until = None, None)
}

pub async fn decrypt(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let pubkey = match args.next() {
        Some(s) => match PublicKey::try_from_hex_string(&s, true) {
            Ok(pk) => pk,
            Err(_) => PublicKey::try_from_bech32_string(&s, true)?,
        },
        None => return cmd.usage("Missing pubkey parameter".to_string()),
    };

    let ciphertext = match args.next() {
        Some(text) => text,
        None => return cmd.usage("Missing ciphertext parameter".to_string()),
    };

    login().await?;

    let plaintext = GLOBALS.identity.decrypt(&pubkey, &ciphertext).await?;
    println!("{}", plaintext);

    Ok(())
}

pub fn delete_by_kind(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let kind: EventKind = match args.next() {
        Some(integer) => integer.parse::<u32>()?.into(),
        None => return cmd.usage("Missing kind parameter".to_string()),
    };

    let mut target_ids: Vec<Id> = Vec::new();

    let mut filter = Filter::new();
    filter.add_event_kind(kind);
    let events = GLOBALS.db().find_events_by_filter(&filter, |_| true)?;
    for event in &events {
        target_ids.push(event.id);
    }

    // Delete locally
    let mut txn = GLOBALS.db().get_write_txn()?;
    for id in &target_ids {
        // Delete locally
        GLOBALS.db().delete_event(*id, Some(&mut txn))?;
    }
    txn.commit()?;

    println!("Deleted {} events", target_ids.len());

    Ok(())
}

pub fn delete_by_id(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let idstr = match args.next() {
        Some(id) => id,
        None => return cmd.usage("Missing idhex parameter".to_string()),
    };

    let id = match Id::try_from_hex_string(&idstr) {
        Ok(id) => id,
        Err(_) => Id::try_from_bech32_string(&idstr)?,
    };

    GLOBALS.db().delete_event(id, None)?;

    Ok(())
}

pub async fn delete_spam_by_content(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let mut kind: EventKind = match args.next() {
        Some(integer) => integer.parse::<u32>()?.into(),
        None => return cmd.usage("Missing kind parameter".to_string()),
    };

    let since = match args.next() {
        Some(s) => Unixtime(s.parse::<i64>()?),
        None => return cmd.usage("Missing <since_unixtime> parameter".to_string()),
    };

    let substring = match args.next() {
        Some(c) => c,
        None => return cmd.usage("Missing <substring> parameter".to_string()),
    };

    // If DM Chat, use GiftWrap
    if kind == EventKind::DmChat {
        kind = EventKind::GiftWrap;
    }

    // Login if we need to look into GiftWraps
    if kind == EventKind::GiftWrap {
        login().await?;
    }

    // Get all event ids of the kind/since
    let mut filter = Filter::new();
    filter.add_event_kind(kind);
    filter.since = Some(since);
    let events = GLOBALS.db().find_events_by_filter(&filter, |_| true)?;

    println!("Searching through {} events...", events.len());

    // Find events among those with matching spammy content
    let mut target_ids: Vec<Id> = Vec::new();
    for event in events {
        let mut matches = false;
        if kind == EventKind::GiftWrap {
            if let Ok(rumor) = GLOBALS.identity.unwrap_giftwrap(&event).await {
                if rumor.content.contains(&substring) {
                    matches = true;
                }
            }
        } else if event.content.contains(&substring) {
            matches = true;
        }

        if matches {
            target_ids.push(event.id);
        }
    }

    // If no events we are done
    if target_ids.is_empty() {
        println!("No matches found");
        return Ok(());
    }

    println!("Deleting {} events...", target_ids.len());

    // Delete locally
    let mut txn = GLOBALS.db().get_write_txn()?;
    for id in &target_ids {
        // Delete locally
        GLOBALS.db().delete_event(*id, Some(&mut txn))?;

        // NOTE: we cannot add a delete relationship; we can't delete
        // other people's events.

        // FIXME: add a database of marked-deleted events
    }
    txn.commit()?;

    // Unless they were giftwraps, we are done
    // (We cannot delete spam on relays that we didn't author unless it is a giftwrap)
    if kind != EventKind::GiftWrap {
        println!("Ok");
        return Ok(());
    }

    // Get the relays these giftwraps were seen on
    let mut relays: HashSet<RelayUrl> = HashSet::new();
    for id in &target_ids {
        // Get seen on relays
        if let Ok(seen_on) = GLOBALS.db().get_event_seen_on_relay(*id) {
            for (relay, _when) in seen_on {
                relays.insert(relay);
            }
        }
    }

    let public_key = GLOBALS.identity.public_key().unwrap();

    // Build up a single deletion event
    let mut tags: Vec<Tag> = Vec::new();
    for id in target_ids {
        tags.push(
            ParsedTag::Event {
                id,
                recommended_relay_url: None,
                marker: None,
                author_pubkey: Some(public_key),
            }
            .into_tag(),
        );
    }
    let event = {
        let pre_event = PreEvent {
            pubkey: public_key,
            created_at: Unixtime::now(),
            kind: EventKind::EventDeletion,
            tags,
            content: "spam".to_owned(),
        };
        // Should we add a pow? Maybe the relay needs it.
        GLOBALS.identity.sign_event(pre_event).await?
    };
    println!("{}", serde_json::to_string(&event).unwrap());

    let job = tokio::task::spawn(async move {
        // Process this event locally
        if let Err(e) =
            gossip_lib::process::process_new_event(&event, None, None, false, false).await
        {
            println!("ERROR: {}", e);
        } else {
            // Post the event to all the relays
            for relay in relays {
                let conn = nostr_types::client::Client::new(relay.as_str());

                match conn
                    .post_event_and_wait_for_result(
                        event.clone(),
                        std::time::Duration::from_secs(3),
                        Some(GLOBALS.identity.inner_lockable().unwrap()),
                    )
                    .await
                {
                    Err(e) => println!("ERROR: {}", e),
                    Ok((true, _)) => {}
                    Ok((false, msg)) => println!("{}", msg),
                }

                let _ = conn.disconnect().await;
            }
        }
    });

    GLOBALS.runtime.block_on(job)?;

    println!("Ok.");
    Ok(())
}

pub fn delete_relay(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let rurl = match args.next() {
        Some(urlstr) => RelayUrl::try_from_str(&urlstr)?,
        None => return cmd.usage("Missing relay url parameter".to_string()),
    };

    GLOBALS.db().delete_relay(&rurl, None)?;

    Ok(())
}

pub fn override_dpi(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let dpi = match args.next() {
        Some(dpistr) => dpistr.parse::<u32>()?,
        None => return cmd.usage("Missing DPI value".to_string()),
    };

    GLOBALS.db().write_setting_override_dpi(&Some(dpi), None)?;

    println!("DPI override setting set to {}", dpi);

    Ok(())
}

pub fn disable_relay(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let rurl = match args.next() {
        Some(urlstr) => RelayUrl::try_from_str(&urlstr)?,
        None => return cmd.usage("Missing relay url parameter".to_string()),
    };

    GLOBALS.db().modify_relay(
        &rurl,
        |relay| {
            relay.rank = 0;
            relay.hidden = true;
        },
        None,
    )?;

    Ok(())
}

pub fn dump_handlers() -> Result<(), Error> {
    use gossip_lib::HandlersTable;

    let mut last_kind = EventKind::Other(12345);

    for (kind, handler_key, enabled, recommended) in
        GLOBALS.db().read_all_configured_handlers()?.iter()
    {
        if *kind != last_kind {
            println!("KIND={:?}", *kind);
            last_kind = *kind;
        }

        if let Some(handler) = HandlersTable::read_record(handler_key.clone(), None)? {
            let handler_url = if kind.is_parameterized_replaceable() {
                if let Some(naddr_url) = &handler.naddr_url {
                    naddr_url.as_str()
                } else {
                    "no web-naddr url provided"
                }
            } else {
                if let Some(nevent_url) = &handler.nevent_url {
                    nevent_url.as_str()
                } else {
                    "no web-nevent url provided"
                }
            };

            println!(
                "  {:?} enabled={} recommended={} url={}",
                handler.bestname(*kind),
                *enabled,
                *recommended,
                handler_url
            );
        }
    }

    Ok(())
}

pub fn import_encrypted_private_key(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let input = match args.next() {
        Some(input) => input,
        None => return cmd.usage("Missing ncryptsec parameter".to_string()),
    };

    let epk = EncryptedPrivateKey(input);

    let mut password = rpassword::prompt_password("Password: ").unwrap();

    GLOBALS.identity.set_encrypted_private_key(epk, &password)?;
    password.zeroize();

    println!("Saved.");
    Ok(())
}

pub async fn import_event(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let event = match args.next() {
        Some(json) => {
            let e: Event = serde_json::from_str(&json)?;
            e
        }
        None => return cmd.usage("Missing event parameter".to_string()),
    };

    login().await?;

    let job = tokio::task::spawn(async move {
        if let Err(e) =
            gossip_lib::process::process_new_event(&event, None, None, false, true).await
        {
            println!("ERROR: {}", e);
        }
    });

    GLOBALS.runtime.block_on(job)?;

    println!("Ok.");
    Ok(())
}

pub fn keys() -> Result<(), Error> {
    match GLOBALS.identity.encrypted_private_key() {
        Some(epk) => println!("epk: {epk}"),
        None => println!("No encrypted private key"),
    };

    match GLOBALS.identity.public_key() {
        Some(pk) => println!("public key: {}", pk.as_bech32_string()),
        None => println!("No public key"),
    };

    Ok(())
}

pub async fn import_lmdb_events(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    use speedy::Readable;
    use std::io::Write;

    let lmdb_str = match args.next() {
        Some(s) => s,
        None => return cmd.usage("Missing other_lmdb_dir parameter".to_string()),
    };

    let storage2 = gossip_lib::Storage::new(lmdb_str, false)?;
    let txn2 = storage2.get_read_txn()?;
    let iter = storage2.event_iterator_rawinit(&txn2)?;

    let mut count = 0;
    let mut stdout = std::io::stdout();
    for result in iter {
        let (_key, bytes) = result?;
        let event = Event::read_from_buffer(bytes)?;
        gossip_lib::process::process_new_event(&event, None, None, false, false).await?;
        count += 1;
        if count % 1000 == 0 {
            print!(".");
            let _ = stdout.flush();
        }
    }

    Ok(())
}

pub fn print_event(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let idstr = match args.next() {
        Some(id) => id,
        None => return cmd.usage("Missing idhex parameter".to_string()),
    };

    let id = Id::try_from_hex_string(&idstr)?;

    match GLOBALS.db().read_event(id)? {
        Some(event) => println!("{}", serde_json::to_string(&event)?),
        None => return Err(ErrorKind::EventNotFound.into()),
    }

    Ok(())
}

pub fn print_relay(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    if let Some(url) = args.next() {
        let rurl = RelayUrl::try_from_str(&url)?;
        if let Some(relay) = GLOBALS.db().read_relay(&rurl)? {
            println!("{}", serde_json::to_string_pretty(&relay)?);
        } else {
            println!("Relay not found.");
        }
        Ok(())
    } else {
        cmd.usage("Missing url parameter".to_string())
    }
}

pub fn print_relays(_cmd: Command) -> Result<(), Error> {
    let relays = GLOBALS.db().filter_relays(|_| true)?;
    for relay in &relays {
        println!("{}", serde_json::to_string(relay)?);
    }
    Ok(())
}

pub fn print_seen_on(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let idstr = match args.next() {
        Some(id) => id,
        None => return cmd.usage("Missing idhex parameter".to_string()),
    };
    let id = Id::try_from_hex_string(&idstr)?;
    for (url, when) in GLOBALS.db().get_event_seen_on_relay(id)? {
        println!("{} at {}", url, when);
    }
    Ok(())
}

pub fn prune_cache() -> Result<(), Error> {
    let age = Duration::new(
        GLOBALS.db().read_setting_cache_prune_period_days() * 60 * 60 * 24,
        0,
    );

    let job = tokio::task::spawn(async move {
        match GLOBALS.fetcher.prune(age).await {
            Ok(count) => println!("Cache has been pruned. {} files removed.", count),
            Err(e) => eprintln!("{e}"),
        }
    });

    GLOBALS.runtime.block_on(job)?;

    Ok(())
}

pub fn prune_old_events() -> Result<(), Error> {
    println!("Pruning miscellaneous tables...");
    GLOBALS.db().prune_misc()?;

    let now = Unixtime::now();
    let then = now
        - Duration::new(
            GLOBALS.db().read_setting_prune_period_days() * 60 * 60 * 24,
            0,
        );

    println!("Pruning old events...");
    let count = GLOBALS.db().prune_old_events(then)?;

    println!("Database has been pruned. {count} events removed.");
    Ok(())
}

pub fn prune_unused_people() -> Result<(), Error> {
    println!("Pruning unused people...");
    let count = GLOBALS.db().prune_unused_people()?;

    println!("Database has been pruned. {count} people removed.");
    Ok(())
}

pub fn print_followed(_cmd: Command) -> Result<(), Error> {
    let members = GLOBALS.db().get_people_in_list(PersonList::Followed)?;
    for (pk, private) in &members {
        if let Some(person) = PersonTable::read_record(*pk, None)? {
            println!(
                "{} {} {}",
                if **private { "prv" } else { "pub" },
                pk.as_hex_string(),
                person.best_name()
            );
        } else {
            println!(
                "{} {}",
                if **private { "prv" } else { "pub" },
                pk.as_hex_string()
            );
        }
    }
    Ok(())
}

pub fn print_muted(_cmd: Command) -> Result<(), Error> {
    let members = GLOBALS.db().get_people_in_list(PersonList::Muted)?;
    for (pk, private) in &members {
        println!(
            "{} {}",
            if **private { "prv" } else { "pub" },
            pk.as_hex_string()
        );
    }
    Ok(())
}

pub fn print_person_lists(_cmd: Command) -> Result<(), Error> {
    let all = GLOBALS.db().get_all_person_list_metadata()?;
    for (list, metadata) in all.iter() {
        println!("LIST {}: {}", u8::from(*list), metadata.title);
        let members = GLOBALS.db().get_people_in_list(*list)?;
        for (pk, private) in &members {
            if let Some(person) = PersonTable::read_record(*pk, None)? {
                println!(
                    "{} {} {}",
                    if **private { "prv" } else { "pub" },
                    pk.as_hex_string(),
                    person.best_name()
                );
            } else {
                println!(
                    "{} {}",
                    if **private { "prv" } else { "pub" },
                    pk.as_hex_string()
                );
            }
        }
        println!();
    }
    Ok(())
}

pub fn print_person(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let pubkey = match args.next() {
        Some(s) => match PublicKey::try_from_hex_string(&s, true) {
            Ok(pk) => pk,
            Err(_) => PublicKey::try_from_bech32_string(&s, true)?,
        },
        None => return cmd.usage("Missing pubkey parameter".to_string()),
    };

    let person = PersonTable::read_record(pubkey, None)?;
    println!("{}", serde_json::to_string(&person)?);
    Ok(())
}

pub fn print_person_relays(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let pubkey = match args.next() {
        Some(s) => match PublicKey::try_from_hex_string(&s, true) {
            Ok(pk) => pk,
            Err(_) => PublicKey::try_from_bech32_string(&s, true)?,
        },
        None => return cmd.usage("Missing pubkey parameter".to_string()),
    };

    let person_relays = GLOBALS.db().get_person_relays(pubkey)?;
    for record in &person_relays {
        println!("{}", serde_json::to_string(record)?);
    }
    Ok(())
}

pub fn events_of_kind(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let kind: EventKind = match args.next() {
        Some(integer) => integer.parse::<u32>()?.into(),
        None => return cmd.usage("Missing kind parameter".to_string()),
    };

    let mut filter = Filter::new();
    filter.add_event_kind(kind);
    let events = GLOBALS.db().find_events_by_filter(&filter, |_| true)?;
    for event in &events {
        println!("{}", serde_json::to_string(event)?);
    }

    Ok(())
}

pub fn events_of_pubkey(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let pubkey = match args.next() {
        Some(s) => match PublicKey::try_from_hex_string(&s, true) {
            Ok(pk) => pk,
            Err(_) => PublicKey::try_from_bech32_string(&s, true)?,
        },
        None => return cmd.usage("Missing pubkey parameter".to_string()),
    };

    let mut filter = Filter::new();
    filter.add_author(pubkey);
    let events = GLOBALS.db().find_events_by_filter(&filter, |_| true)?;

    for event in &events {
        println!("{}", serde_json::to_string(event)?);
    }

    Ok(())
}

pub fn events_of_pubkey_and_kind(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let pubkey = match args.next() {
        Some(s) => match PublicKey::try_from_hex_string(&s, true) {
            Ok(pk) => pk,
            Err(_) => PublicKey::try_from_bech32_string(&s, true)?,
        },
        None => return cmd.usage("Missing pubkey parameter".to_string()),
    };

    let kind: EventKind = match args.next() {
        Some(integer) => integer.parse::<u32>()?.into(),
        None => return cmd.usage("Missing kind parameter".to_string()),
    };

    let mut filter = Filter::new();
    filter.add_event_kind(kind);
    filter.add_author(pubkey);
    let events = GLOBALS.db().find_events_by_filter(&filter, |_| true)?;

    for event in &events {
        println!("{}", serde_json::to_string(event)?);
    }

    Ok(())
}

pub fn export_encrypted_key() -> Result<(), Error> {
    let epk = match GLOBALS.identity.encrypted_private_key() {
        Some(epk) => epk,
        None => return Err(ErrorKind::NoPrivateKey.into()),
    };

    println!("{}", epk);

    Ok(())
}

pub fn force_migration_level(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let level = match args.next() {
        Some(l) => l.parse::<u32>()?,
        None => return cmd.usage("Missing level parameter".to_string()),
    };

    GLOBALS.db().force_migration_level(level)?;

    Ok(())
}

pub async fn ungiftwrap(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let idstr = match args.next() {
        Some(id) => id,
        None => return cmd.usage("Missing idhex parameter".to_string()),
    };

    let id = Id::try_from_hex_string(&idstr)?;

    let event = match GLOBALS.db().read_event(id)? {
        Some(event) => {
            if event.kind != EventKind::GiftWrap {
                return Err(ErrorKind::WrongEventKind.into());
            } else {
                event
            }
        }
        None => return Err(ErrorKind::EventNotFound.into()),
    };

    login().await?;

    let rumor = GLOBALS.identity.unwrap_giftwrap(&event).await?;

    println!("{}", serde_json::to_string(&rumor)?);

    Ok(())
}

pub async fn giftwraps(_cmd: Command) -> Result<(), Error> {
    login().await?;

    let mut filter = Filter::new();
    filter.add_event_kind(EventKind::GiftWrap);
    let events = GLOBALS.db().find_events_by_filter(&filter, |_| true)?;

    for event in &events {
        println!(
            "giftwrap_id={} giftwrap_created_at={}",
            event.id.as_hex_string(),
            event.created_at
        );
        match GLOBALS.identity.unwrap_giftwrap(event).await {
            Ok(rumor) => println!("{}", serde_json::to_string(&rumor)?),
            Err(e) => println!("  {}", e),
        }
    }

    Ok(())
}

pub fn reaction_stats(_cmd: Command, mut _args: env::Args) -> Result<(), Error> {
    use std::collections::HashMap;
    let mut reactions: HashMap<String, usize> = HashMap::new();
    let mut filter = Filter::new();
    filter.add_event_kind(EventKind::Reaction);
    let events = GLOBALS.db().find_events_by_filter(&filter, |_| true)?;
    for event in events {
        reactions
            .entry(event.content.clone())
            .and_modify(|count| *count += 1)
            .or_insert(1);
    }
    let mut reactions: Vec<(String, usize)> = reactions.drain().collect();
    reactions.sort_by(|a, b| b.1.cmp(&a.1));
    for (reaction, count) in reactions {
        println!("{} {}", count, reaction);
    }
    Ok(())
}

pub fn rebuild_fof() -> Result<(), Error> {
    GLOBALS.db().set_flag_rebuild_fof_needed(true, None)?;
    println!("Friends of friends data will be rebuilt next time gossip starts.");
    Ok(())
}

pub async fn reprocess_recent(_cmd: Command) -> Result<(), Error> {
    login().await?;

    let job = tokio::task::spawn(async move {
        let mut ago = Unixtime::now();
        ago.0 -= 86400;

        let mut filter = Filter::new();
        filter.kinds = EventKind::iter().collect(); // all kinds
        filter.since = Some(ago);
        let events = match GLOBALS.db().find_events_by_filter(&filter, |_| true) {
            Ok(e) => e,
            Err(e) => {
                println!("ERROR: {}", e);
                vec![]
            }
        };

        let mut count = 0;
        for event in events.iter() {
            if let Err(e) =
                gossip_lib::process::process_new_event(event, None, None, false, true).await
            {
                println!("ERROR: {}", e);
            }
            count += 1;
            if count % 100 == 0 {
                println!("{}...", count);
            }
        }

        println!("Done.");
    });

    Ok(GLOBALS.runtime.block_on(job)?)
}

pub fn reprocess_relay_lists() -> Result<(), Error> {
    let (c1, c2) = gossip_lib::process::reprocess_relay_lists()?;
    println!("Reprocessed {} contact lists", c1);
    println!("Reprocessed {} relay lists", c2);
    Ok(())
}

pub fn reset_relay_auth() -> Result<(), Error> {
    GLOBALS.db().modify_all_relays(
        |relay| {
            relay.allow_auth = None;
        },
        None,
    )?;
    Ok(())
}

pub fn reset_relay_connect() -> Result<(), Error> {
    GLOBALS.db().modify_all_relays(
        |relay| {
            relay.allow_connect = None;
        },
        None,
    )?;
    Ok(())
}

pub fn set_theme(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let theme = match args.next() {
        Some(s) => s,
        None => return cmd.usage("Missing theme selection".to_string()),
    };

    match theme.as_str() {
        "dark" => {
            GLOBALS.db().write_setting_dark_mode(&true, None)?;
            println!("Setting 'dark' theme");
        }
        "light" => {
            GLOBALS.db().write_setting_dark_mode(&false, None)?;
            println!("Setting 'light' theme");
        }
        _ => return cmd.usage("Invalid theme selection".to_string()),
    };

    Ok(())
}

pub fn verify(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let idstr = match args.next() {
        Some(id) => id,
        None => return cmd.usage("Missing idhex parameter".to_string()),
    };

    let id = Id::try_from_hex_string(&idstr)?;

    match GLOBALS.db().read_event(id)? {
        Some(event) => {
            event.verify(None)?;
            println!("Valid event");
        }
        None => return Err(ErrorKind::EventNotFound.into()),
    }

    Ok(())
}

pub fn verify_json(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let json = match args.next() {
        Some(json) => json,
        None => return cmd.usage("Missing json parameter".to_string()),
    };

    let event: Event = serde_json::from_str(&json)?;
    event.verify(None)?;
    println!("Valid event");

    Ok(())
}

pub async fn rebuild_indices() -> Result<(), Error> {
    println!("Login required in order to reindex DMs and GiftWraps");
    login().await?;
    GLOBALS.db().rebuild_event_indices(None).await?;

    Ok(())
}

pub fn rename_person_list(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let number: u8 = match args.next() {
        Some(number) => number.parse::<u8>()?,
        None => return cmd.usage("Missing number parameter".to_string()),
    };

    let newname = match args.next() {
        Some(name) => name,
        None => return cmd.usage("Missing newname parameter".to_string()),
    };

    let list = match PersonList::from_number(number) {
        Some(list) => list,
        None => {
            println!("No list with number={}", number);
            return Ok(());
        }
    };

    GLOBALS.db().rename_person_list(list, newname, None)?;

    Ok(())
}

pub async fn login() -> Result<(), Error> {
    if !GLOBALS.identity.is_unlocked() {
        let mut password = rpassword::prompt_password("Password: ").unwrap();
        if GLOBALS.identity.can_sign_if_unlocked() {
            GLOBALS.identity.unlock(&password).await?;
        } else {
            return Err(ErrorKind::IdentityCannotSign.into());
        }
        password.zeroize();
    } else {
        println!("No private key, skipping login");
    }
    Ok(())
}

pub fn offline() -> Result<(), Error> {
    GLOBALS.db().write_setting_offline(&true, None)?;
    Ok(())
}

pub fn wgpu_renderer(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let enable = match args.next() {
        Some(str) => str.parse::<bool>()?,
        None => return cmd.usage("Missing true|false value".to_string()),
    };

    GLOBALS.db().write_setting_wgpu_renderer(&enable, None)?;

    println!("wgpu_renderer set to '{}'", enable);

    Ok(())
}

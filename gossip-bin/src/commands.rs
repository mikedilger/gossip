use gossip_lib::{Error, ErrorKind, PersonList, PersonListMetadata, PersonTable, Table, GLOBALS};
use nostr_types::{
    EncryptedPrivateKey, Event, NAddr, EventKind, Filter, Id, NostrBech32, NostrUrl, PreEvent,
    PrivateKey, PublicKey, RelayUrl, Tag, UncheckedUrl, Unixtime,
};
use std::collections::HashSet;
use std::env;
use tokio::runtime::Runtime;
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

const COMMANDS: [Command; 36] = [
    Command {
        cmd: "oneshot",
        usage_params: "{depends}",
        desc: "temporary oneshot action",
    },
    Command {
        cmd: "add_person_list",
        usage_params: "<listname>",
        desc: "add a new person list with the given name",
    },
    Command {
        cmd: "backdate_eose",
        usage_params: "",
        desc: "backdate last_general_eose_at by 24 hours for every relay",
    },
    Command {
        cmd: "bech32_decode",
        usage_params: "<bech32string>",
        desc: "decode the bech32 string.",
    },
    Command {
        cmd: "bech32_encode_naddr",
        usage_params: "<kind> <pubkeyhex> <d> [<relayurl>, ...]",
        desc: "encode an event address (parameterized replaceable event link).",
    },
    Command {
        cmd: "clear_timeouts",
        usage_params: "",
        desc: "clear relay avoidance timeouts.",
    },
    Command {
        cmd: "decrypt",
        usage_params: "<pubkeyhex> <ciphertext>",
        desc: "decrypt the ciphertext from the pubkeyhex.",
    },
    Command {
        cmd: "delete_spam_by_content",
        usage_params: "<kind> <unixtime_since> <substring>",
        desc: "delete all feed-displayable events with content matching the substring (including giftwraps)",
    },
    Command {
        cmd: "delete_relay",
        usage_params: "<relayurl>",
        desc: "delete a relay record from storage.",
    },
    Command {
        cmd: "dpi",
        usage_params: "<dpi>",
        desc: "override the DPI setting",
    },
    Command {
        cmd: "events_of_kind",
        usage_params: "<kind>",
        desc: "print IDs of all events of kind=<kind>",
    },
    Command {
        cmd: "events_of_pubkey_and_kind",
        usage_params: "<pubkeyhex> <kind>",
        desc: "print IDs of all events from <pubkeyhex> of kind=<kind>",
    },
    Command {
        cmd: "export_encrypted_key",
        usage_params: "",
        desc: "Export the encrypted private key",
    },
    Command {
        cmd: "giftwraps",
        usage_params: "",
        desc: "Decrypt all of your giftwraps",
    },
    Command {
        cmd: "help",
        usage_params: "<command>",
        desc: "show this list",
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
        cmd: "login",
        usage_params: "",
        desc: "login on the command line before starting the gossip GUI",
    },
    Command {
        cmd: "offline",
        usage_params: "",
        desc: "login on the command line before starting the gossip GUI",
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
        usage_params: "<pubkeyHexOrBech32>",
        desc: "print the given person",
    },
    Command {
        cmd: "print_person_relays",
        usage_params: "<pubkeyhex>",
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
    }
];

pub fn handle_command(mut args: env::Args, runtime: &Runtime) -> Result<bool, Error> {
    let _ = args.next(); // program name
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
        "decrypt" => decrypt(command, args)?,
        "delete_spam_by_content" => delete_spam_by_content(command, args, runtime)?,
        "delete_relay" => delete_relay(command, args)?,
        "dpi" => override_dpi(command, args)?,
        "events_of_kind" => events_of_kind(command, args)?,
        "events_of_pubkey_and_kind" => events_of_pubkey_and_kind(command, args)?,
        "export_encrypted_key" => export_encrypted_key()?,
        "giftwraps" => giftwraps(command)?,
        "help" => help(command, args)?,
        "import_encrypted_private_key" => import_encrypted_private_key(command, args)?,
        "import_event" => import_event(command, args, runtime)?,
        "login" => {
            login()?;
            return Ok(false);
        }
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
        "rebuild_indices" => rebuild_indices()?,
        "rename_person_list" => rename_person_list(command, args)?,
        "reprocess_recent" => reprocess_recent(command, runtime)?,
        "reprocess_relay_lists" => reprocess_relay_lists()?,
        "ungiftwrap" => ungiftwrap(command, args)?,
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
                println!("gossip {} {}", c.cmd, c.usage_params);
                println!("    {}", c.desc);
                return Ok(());
            }
        }
        println!("No such command {}", sub);
    } else {
        for c in COMMANDS.iter() {
            println!("  {} {}", c.cmd, c.usage_params);
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

    let _list = GLOBALS.storage.allocate_person_list(&metadata, None)?;
    Ok(())
}

pub fn backdate_eose() -> Result<(), Error> {
    let now = Unixtime::now();
    let ago = (now.0 - 60 * 60 * 24) as u64;

    GLOBALS.storage.modify_all_relays(
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
        Some(hex) => PublicKey::try_from_hex_string(&hex, true)?,
        None => return cmd.usage("Missing pubkeyhex parameter".to_string()),
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
        .storage
        .modify_all_relays(|r| r.avoid_until = None, None)
}

pub fn decrypt(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let pubkey = match args.next() {
        Some(hex) => PublicKey::try_from_hex_string(&hex, true)?,
        None => return cmd.usage("Missing pubkeyhex parameter".to_string()),
    };

    let ciphertext = match args.next() {
        Some(text) => text,
        None => return cmd.usage("Missing ciphertext parameter".to_string()),
    };

    login()?;

    let plaintext = GLOBALS.identity.decrypt(&pubkey, &ciphertext)?;
    println!("{}", plaintext);

    Ok(())
}

pub fn delete_spam_by_content(
    cmd: Command,
    mut args: env::Args,
    runtime: &Runtime,
) -> Result<(), Error> {
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
        login()?;
    }

    // Get all event ids of the kind/since
    let mut filter = Filter::new();
    filter.add_event_kind(kind);
    filter.since = Some(since);
    let events = GLOBALS.storage.find_events_by_filter(&filter, |_| true)?;

    println!("Searching through {} events...", events.len());

    // Find events among those with matching spammy content
    let mut target_ids: Vec<Id> = Vec::new();
    for event in events {
        let mut matches = false;
        if kind == EventKind::GiftWrap {
            if let Ok(rumor) = GLOBALS.identity.unwrap_giftwrap(&event) {
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
    let mut txn = GLOBALS.storage.get_write_txn()?;
    for id in &target_ids {
        // Delete locally
        GLOBALS.storage.delete_event(*id, Some(&mut txn))?;

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
        if let Ok(seen_on) = GLOBALS.storage.get_event_seen_on_relay(*id) {
            for (relay, _when) in seen_on {
                relays.insert(relay);
            }
        }
    }

    // Build up a single deletion event
    let mut tags: Vec<Tag> = Vec::new();
    for id in target_ids {
        tags.push(Tag::new_event(id, None, None));
    }
    let event = {
        let public_key = GLOBALS.identity.public_key().unwrap();
        let pre_event = PreEvent {
            pubkey: public_key,
            created_at: Unixtime::now(),
            kind: EventKind::EventDeletion,
            tags,
            content: "spam".to_owned(),
        };
        // Should we add a pow? Maybe the relay needs it.
        GLOBALS.identity.sign_event(pre_event)?
    };
    println!("{}", serde_json::to_string(&event).unwrap());

    let job = tokio::task::spawn(async move {
        // Process this event locally
        if let Err(e) = gossip_lib::process::process_new_event(&event, None, None, false, false) {
            println!("ERROR: {}", e);
        } else {
            // Post the event to all the relays
            for relay in relays {
                if let Err(e) = gossip_lib::direct::post(relay.as_str(), event.clone()) {
                    println!("ERROR: {}", e);
                }
            }
        }
    });

    runtime.block_on(job)?;

    println!("Ok.");
    Ok(())
}

pub fn delete_relay(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let rurl = match args.next() {
        Some(urlstr) => RelayUrl::try_from_str(&urlstr)?,
        None => return cmd.usage("Missing relay url parameter".to_string()),
    };

    GLOBALS.storage.delete_relay(&rurl, None)?;

    Ok(())
}

pub fn override_dpi(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let dpi = match args.next() {
        Some(dpistr) => dpistr.parse::<u32>()?,
        None => return cmd.usage("Missing DPI value".to_string()),
    };

    GLOBALS
        .storage
        .write_setting_override_dpi(&Some(dpi), None)?;

    println!("DPI override setting set to {}", dpi);

    Ok(())
}

pub fn import_encrypted_private_key(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let input = match args.next() {
        Some(input) => input,
        None => return cmd.usage("Missing ncryptsec parameter".to_string()),
    };

    let epk = EncryptedPrivateKey(input);

    // Verify first
    let mut password = rpassword::prompt_password("Password: ").unwrap();
    let _private_key = epk.decrypt(&password)?;
    password.zeroize();

    GLOBALS
        .storage
        .write_encrypted_private_key(Some(&epk), None)?;

    println!("Saved.");
    Ok(())
}

pub fn import_event(cmd: Command, mut args: env::Args, runtime: &Runtime) -> Result<(), Error> {
    let event = match args.next() {
        Some(json) => {
            let e: Event = serde_json::from_str(&json)?;
            e
        }
        None => return cmd.usage("Missing event parameter".to_string()),
    };

    login()?;

    let job = tokio::task::spawn(async move {
        if let Err(e) = gossip_lib::process::process_new_event(&event, None, None, false, true) {
            println!("ERROR: {}", e);
        }
    });

    runtime.block_on(job)?;

    println!("Ok.");
    Ok(())
}

pub fn print_event(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let idstr = match args.next() {
        Some(id) => id,
        None => return cmd.usage("Missing idhex parameter".to_string()),
    };

    let id = Id::try_from_hex_string(&idstr)?;

    match GLOBALS.storage.read_event(id)? {
        Some(event) => println!("{}", serde_json::to_string(&event)?),
        None => return Err(ErrorKind::EventNotFound.into()),
    }

    Ok(())
}

pub fn print_relay(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    if let Some(url) = args.next() {
        let rurl = RelayUrl::try_from_str(&url)?;
        if let Some(relay) = GLOBALS.storage.read_relay(&rurl, None)? {
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
    let relays = GLOBALS.storage.filter_relays(|_| true)?;
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
    for (url, when) in GLOBALS.storage.get_event_seen_on_relay(id)? {
        println!("{} at {}", url, when);
    }
    Ok(())
}

pub fn print_followed(_cmd: Command) -> Result<(), Error> {
    let members = GLOBALS.storage.get_people_in_list(PersonList::Followed)?;
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
    let members = GLOBALS.storage.get_people_in_list(PersonList::Muted)?;
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
    let all = GLOBALS.storage.get_all_person_list_metadata()?;
    for (list, metadata) in all.iter() {
        println!("LIST {}: {}", u8::from(*list), metadata.title);
        let members = GLOBALS.storage.get_people_in_list(*list)?;
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
        None => return cmd.usage("Missing pubkeyHexOrBech32 parameter".to_string()),
    };

    let person = PersonTable::read_record(pubkey, None)?;
    println!("{}", serde_json::to_string(&person)?);
    Ok(())
}

pub fn print_person_relays(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let pubkey = match args.next() {
        Some(hex) => PublicKey::try_from_hex_string(&hex, true)?,
        None => return cmd.usage("Missing pubkeyhex parameter".to_string()),
    };

    let person_relays = GLOBALS.storage.get_person_relays(pubkey)?;
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
    let events = GLOBALS.storage.find_events_by_filter(&filter, |_| true)?;
    for event in events {
        println!("{}", event.id.as_hex_string());
    }

    Ok(())
}

pub fn events_of_pubkey_and_kind(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let pubkey = match args.next() {
        Some(hex) => PublicKey::try_from_hex_string(&hex, true)?,
        None => return cmd.usage("Missing pubkeyhex parameter".to_string()),
    };

    let kind: EventKind = match args.next() {
        Some(integer) => integer.parse::<u32>()?.into(),
        None => return cmd.usage("Missing kind parameter".to_string()),
    };

    let mut filter = Filter::new();
    filter.add_event_kind(kind);
    filter.add_author(&pubkey.into());
    let events = GLOBALS.storage.find_events_by_filter(&filter, |_| true)?;

    for event in events {
        println!("{}", event.id.as_hex_string());
    }

    Ok(())
}

pub fn export_encrypted_key() -> Result<(), Error> {
    let epk = match GLOBALS.storage.read_encrypted_private_key()? {
        Some(epk) => epk,
        None => return Err(ErrorKind::NoPrivateKey.into()),
    };

    println!("{}", epk);

    Ok(())
}

pub fn ungiftwrap(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let idstr = match args.next() {
        Some(id) => id,
        None => return cmd.usage("Missing idhex parameter".to_string()),
    };

    let id = Id::try_from_hex_string(&idstr)?;

    let event = match GLOBALS.storage.read_event(id)? {
        Some(event) => {
            if event.kind != EventKind::GiftWrap {
                return Err(ErrorKind::WrongEventKind.into());
            } else {
                event
            }
        }
        None => return Err(ErrorKind::EventNotFound.into()),
    };

    login()?;

    let rumor = GLOBALS.identity.unwrap_giftwrap(&event)?;

    println!("{}", serde_json::to_string(&rumor)?);

    Ok(())
}

pub fn giftwraps(_cmd: Command) -> Result<(), Error> {
    login()?;

    let mut filter = Filter::new();
    filter.add_event_kind(EventKind::GiftWrap);
    let events = GLOBALS.storage.find_events_by_filter(&filter, |_| true)?;

    for event in &events {
        println!(
            "giftwrap_id={} giftwrap_created_at={}",
            event.id.as_hex_string(),
            event.created_at
        );
        match GLOBALS.identity.unwrap_giftwrap(event) {
            Ok(rumor) => println!("{}", serde_json::to_string(&rumor)?),
            Err(e) => println!("  {}", e),
        }
    }

    Ok(())
}

pub fn reprocess_recent(_cmd: Command, runtime: &Runtime) -> Result<(), Error> {
    login()?;

    let job = tokio::task::spawn(async move {
        let mut ago = Unixtime::now();
        ago.0 -= 86400;

        let mut filter = Filter::new();
        filter.kinds = EventKind::iter().collect(); // all kinds
        filter.since = Some(ago);
        let events = match GLOBALS.storage.find_events_by_filter(&filter, |_| true) {
            Ok(e) => e,
            Err(e) => {
                println!("ERROR: {}", e);
                vec![]
            }
        };

        let mut count = 0;
        for event in events.iter() {
            if let Err(e) = gossip_lib::process::process_new_event(event, None, None, false, true) {
                println!("ERROR: {}", e);
            }
            count += 1;
            if count % 100 == 0 {
                println!("{}...", count);
            }
        }

        println!("Done.");
    });

    Ok(runtime.block_on(job)?)
}

pub fn reprocess_relay_lists() -> Result<(), Error> {
    let (c1, c2) = gossip_lib::process::reprocess_relay_lists()?;
    println!("Reprocessed {} contact lists", c1);
    println!("Reprocessed {} relay lists", c2);
    Ok(())
}

pub fn verify(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let idstr = match args.next() {
        Some(id) => id,
        None => return cmd.usage("Missing idhex parameter".to_string()),
    };

    let id = Id::try_from_hex_string(&idstr)?;

    match GLOBALS.storage.read_event(id)? {
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

pub fn rebuild_indices() -> Result<(), Error> {
    println!("Login required in order to reindex DMs and GiftWraps");
    login()?;
    GLOBALS.storage.rebuild_event_indices(None)?;

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

    GLOBALS.storage.rename_person_list(list, newname, None)?;

    Ok(())
}

pub fn login() -> Result<(), Error> {
    if !GLOBALS.identity.is_unlocked() {
        let mut password = rpassword::prompt_password("Password: ").unwrap();
        if !GLOBALS.identity.has_private_key() {
            let epk = match GLOBALS.storage.read_encrypted_private_key()? {
                Some(epk) => epk,
                None => return Err(ErrorKind::NoPrivateKey.into()),
            };
            GLOBALS.identity.set_encrypted_private_key(epk, &password)?;
        } else {
            GLOBALS.identity.unlock(&password)?;
        }
        password.zeroize();
    } else {
        println!("No private key, skipping login");
    }
    Ok(())
}

pub fn offline() -> Result<(), Error> {
    GLOBALS.storage.write_setting_offline(&true, None)?;
    Ok(())
}

pub fn wgpu_renderer(cmd: Command, mut args: env::Args) -> Result<(), Error> {
    let enable = match args.next() {
        Some(str) => str.parse::<bool>()?,
        None => return cmd.usage("Missing true|false value".to_string()),
    };

    GLOBALS.storage.write_setting_wgpu_renderer(&enable, None)?;

    println!("wgpu_renderer set to '{}'", enable);

    Ok(())
}

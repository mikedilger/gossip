use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use nostr_types::{Event, EventKind, Id, PublicKey, Unixtime};
use std::env;
use tokio::runtime::Runtime;
use zeroize::Zeroize;

pub fn handle_command(mut args: env::Args, runtime: &Runtime) -> Result<bool, Error> {
    let _ = args.next(); // program name
    let command = args.next().unwrap(); // must be there or we would not have been called

    println!("\n*** Gossip is running in command mode ***");
    println!("*** COMMAND = {} ***\n", command);

    match &*command {
        "decrypt" => decrypt(args)?,
        "dump_event" => dump_event(args)?,
        "dump_person_relays" => dump_person_relays(args)?,
        "dump_relays" => dump_relays()?,
        "events_of_kind" => events_of_kind(args)?,
        "events_of_pubkey_and_kind" => events_of_pubkey_and_kind(args)?,
        "giftwrap_ids" => giftwrap_ids()?,
        "help" => help()?,
        "login" => {
            login()?;
            return Ok(false);
        }
        "rebuild_indices" => rebuild_indices()?,
        "reprocess_recent" => reprocess_recent(runtime)?,
        "ungiftwrap" => ungiftwrap(args)?,
        "verify" => verify(args)?,
        "verify_json" => verify_json(args)?,
        other => println!("Unknown command {}", other),
    }

    Ok(true)
}

pub fn help() -> Result<(), Error> {
    println!("gossip decrypt <pubkeyhex> <ciphertext> <padded?>");
    println!("    decrypt the ciphertext from the pubkeyhex. padded=0 to not expect padding.");
    println!("gossip dump_event <idhex>");
    println!("    print the event (in JSON) from the database that has the given id");
    println!("gossip dump_person_relays <pubkeyhex>");
    println!("    print all the person-relay records for the given person");
    println!("gossip dump_relays");
    println!("    print all the relay records");
    println!("gossip events_of_kind <kind>");
    println!("    print IDs of all events of kind=<kind>");
    println!("gossip events_of_pubkey_and_kind <pubkeyhex> <kind>");
    println!("    print IDs of all events from <pubkeyhex> of kind=<kind>");
    println!("gossip giftwrap_ids");
    println!("    List the IDs of all giftwrap events you are tagged on");
    println!("gossip help");
    println!("    show this list");
    println!("gossip login");
    println!("    login on the command line before starting the gossip GUI");
    println!("gossip rebuild_indices");
    println!("    Rebuild all event-related indices");
    println!("gossip reprocess_recent");
    println!("    Reprocess events that came during the last 24 hours");
    println!("gossip ungiftwrap <idhex>");
    println!("    Unwrap the giftwrap event with the given ID and print the rumor (in JSON)");
    println!("gossip verify <idhex>");
    println!("    Verify if the given event signature is valid");
    println!("gossip verify_json <event_json>");
    println!("    Verify if the passed in event JSON's signature is valid");

    Ok(())
}

pub fn decrypt(mut args: env::Args) -> Result<(), Error> {
    let pubkey = match args.next() {
        Some(hex) => PublicKey::try_from_hex_string(&hex, true)?,
        None => {
            return Err(ErrorKind::Usage(
                "Missing pubkeyhex parameter".to_string(),
                "decrypt <pubkeyhex> <ciphertext> <padded?>".to_owned(),
            )
            .into())
        }
    };

    let ciphertext = match args.next() {
        Some(text) => text,
        None => {
            return Err(ErrorKind::Usage(
                "Missing ciphertext parameter".to_string(),
                "decrypt <pubkeyhex> <ciphertext> <padded?>".to_owned(),
            )
            .into())
        }
    };

    let padded = match args.next() {
        Some(padded) => padded == "1",
        None => {
            return Err(ErrorKind::Usage(
                "Missing padded parameter".to_string(),
                "decrypt <pubkeyhex> <ciphertext> <padded?>".to_owned(),
            )
            .into())
        }
    };

    login()?;

    let plaintext_bytes = GLOBALS.signer.nip44_decrypt(&pubkey, &ciphertext, padded)?;
    let plaintext = String::from_utf8_lossy(&plaintext_bytes);
    println!("{}", plaintext);

    Ok(())
}

pub fn dump_event(mut args: env::Args) -> Result<(), Error> {
    let idstr = match args.next() {
        Some(id) => id,
        None => {
            return Err(ErrorKind::Usage(
                "Missing idhex parameter".to_string(),
                "dump_event <idhex>".to_owned(),
            )
            .into())
        }
    };

    let id = Id::try_from_hex_string(&idstr)?;

    match GLOBALS.storage.read_event(id)? {
        Some(event) => println!("{}", serde_json::to_string(&event)?),
        None => return Err(ErrorKind::EventNotFound.into()),
    }

    Ok(())
}

pub fn dump_relays() -> Result<(), Error> {
    let relays = GLOBALS.storage.filter_relays(|_| true)?;
    for relay in &relays {
        println!("{:?}", relay);
    }
    Ok(())
}

pub fn dump_person_relays(mut args: env::Args) -> Result<(), Error> {
    let pubkey = match args.next() {
        Some(hex) => PublicKey::try_from_hex_string(&hex, true)?,
        None => {
            return Err(ErrorKind::Usage(
                "Missing pubkeyhex parameter".to_string(),
                "dump_person_relays <pubkeyhex>".to_owned(),
            )
            .into())
        }
    };

    let person_relays = GLOBALS.storage.get_person_relays(pubkey)?;
    for record in &person_relays {
        println!("write={} read={}, mwrite={}, mread={}, url={} last_fetched={:?}, last_suggested_kind3={:?}, last_suggested_nip05={:?}, last_suggested_by_tag={:?}",
                 record.write as usize, record.read as usize,
                 record.manually_paired_write as usize, record.manually_paired_read as usize,
                 record.url,
                 record.last_fetched,
                 record.last_suggested_kind3,
                 record.last_suggested_nip05,
                 record.last_suggested_bytag);
    }
    Ok(())
}

pub fn events_of_kind(mut args: env::Args) -> Result<(), Error> {
    let kind: EventKind = match args.next() {
        Some(integer) => integer.parse::<u32>()?.into(),
        None => {
            return Err(ErrorKind::Usage(
                "Missing kind parameter".to_string(),
                "events_of_kind <kind>".to_owned(),
            )
            .into())
        }
    };

    let ids = GLOBALS.storage.find_event_ids(&[kind], &[], None)?;

    for id in ids {
        println!("{}", id.as_hex_string());
    }

    Ok(())
}

pub fn events_of_pubkey_and_kind(mut args: env::Args) -> Result<(), Error> {
    let pubkey = match args.next() {
        Some(hex) => PublicKey::try_from_hex_string(&hex, true)?,
        None => {
            return Err(ErrorKind::Usage(
                "Missing pubkeyhex parameter".to_string(),
                "events_of_pubkey_and_kind <pubkeyhex> <kind>".to_owned(),
            )
            .into())
        }
    };

    let kind: EventKind = match args.next() {
        Some(integer) => integer.parse::<u32>()?.into(),
        None => {
            return Err(ErrorKind::Usage(
                "Missing kind parameter".to_string(),
                "events_of_pubkey_and_kind <pubkeyhex> <kind>".to_owned(),
            )
            .into())
        }
    };

    let ids = GLOBALS.storage.find_event_ids(&[kind], &[pubkey], None)?;

    for id in ids {
        println!("{}", id.as_hex_string());
    }

    Ok(())
}

pub fn ungiftwrap(mut args: env::Args) -> Result<(), Error> {
    let idstr = match args.next() {
        Some(id) => id,
        None => {
            return Err(ErrorKind::Usage(
                "Missing idhex parameter".to_string(),
                "ungiftwrap <idhex>".to_owned(),
            )
            .into())
        }
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

    let rumor = GLOBALS.signer.unwrap_giftwrap(&event)?;

    println!("{}", serde_json::to_string(&rumor)?);

    Ok(())
}

pub fn giftwrap_ids() -> Result<(), Error> {
    let ids = GLOBALS
        .storage
        .find_event_ids(&[EventKind::GiftWrap], &[], None)?;

    for id in ids {
        println!("{}", id.as_hex_string());
    }

    Ok(())
}

pub fn reprocess_recent(runtime: &Runtime) -> Result<(), Error> {
    login()?;

    let job = tokio::task::spawn(async move {
        let all_kinds: Vec<EventKind> = EventKind::iter().collect();

        let mut ago = Unixtime::now().unwrap();
        ago.0 -= 86400;

        let events = match GLOBALS
            .storage
            .find_events(&*all_kinds, &[], Some(ago), |_| true, false)
        {
            Ok(e) => e,
            Err(e) => {
                println!("ERROR: {}", e);
                vec![]
            }
        };

        let mut count = 0;
        for event in events.iter() {
            if let Err(e) = crate::process::process_new_event(&event, None, None, false, true).await
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

    Ok(runtime.block_on(job)?)
}

pub fn verify(mut args: env::Args) -> Result<(), Error> {
    let idstr = match args.next() {
        Some(id) => id,
        None => {
            return Err(ErrorKind::Usage(
                "Missing idhex parameter".to_string(),
                "verify <idhex>".to_owned(),
            )
            .into())
        }
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

pub fn verify_json(mut args: env::Args) -> Result<(), Error> {
    let json = match args.next() {
        Some(json) => json,
        None => {
            return Err(ErrorKind::Usage(
                "Missing json parameter".to_string(),
                "verify_json <event_json>".to_owned(),
            )
            .into())
        }
    };

    let event: Event = serde_json::from_str(&json)?;
    event.verify(None)?;
    println!("Valid event");

    Ok(())
}

pub fn rebuild_indices() -> Result<(), Error> {
    login()?;

    GLOBALS.storage.rebuild_event_indices()?;
    Ok(())
}

pub fn login() -> Result<(), Error> {
    let mut password = rpassword::prompt_password("Password: ").unwrap();
    let epk = match GLOBALS.storage.read_encrypted_private_key()? {
        Some(epk) => epk,
        None => return Err(ErrorKind::NoPrivateKey.into()),
    };
    GLOBALS.signer.set_encrypted_private_key(epk);
    GLOBALS.signer.unlock_encrypted_private_key(&password)?;
    password.zeroize();
    Ok(())
}

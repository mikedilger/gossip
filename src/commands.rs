use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use nostr_types::{Event, EventKind, Id, PublicKey};
use std::env;
use zeroize::Zeroize;

pub fn handle_command(mut args: env::Args) -> Result<bool, Error> {
    let _ = args.next(); // program name
    let command = args.next().unwrap(); // must be there or we would not have been called

    println!("\n*** Gossip is running in command mode ***");
    println!("*** COMMAND = {} ***\n", command);

    match &*command {
        "decrypt" => decrypt(args)?,
        "dump_event" => dump_event(args)?,
        "help" => help()?,
        "login" => {
            login()?;
            return Ok(false);
        }
        "ungiftwrap" => ungiftwrap(args)?,
        "giftwrap_ids" => giftwrap_ids()?,
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
    println!("gossip help");
    println!("    show this list");
    println!("gossip login");
    println!("    login on the command line before starting the gossip GUI");
    println!("gossip ungiftwrap <idhex>");
    println!("    Unwrap the giftwrap event with the given ID and print the rumor (in JSON)");
    println!("gossip giftwrap_ids");
    println!("    List the IDs of all giftwrap events you are tagged on");
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
                "Missing ciphertext parameter".to_string(),
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
                "Missing ciphertext parameter".to_string(),
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
    login()?;

    let ids = GLOBALS
        .storage
        .find_event_ids(&[EventKind::GiftWrap], &[], None)?;

    for id in ids {
        println!("{}", id.as_hex_string());
    }

    Ok(())
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

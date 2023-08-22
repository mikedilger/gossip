use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use nostr_types::{EventKind, Id, PublicKey};
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
        "login" => {
            login()?;
            return Ok(false);
        }
        "test" => println!("Test successful"),
        "ungiftwrap" => ungiftwrap(args)?,
        other => println!("Unknown command {}", other),
    }

    Ok(true)
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
                "ungiftwrap <idhex> <encprivkey>".to_owned(),
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

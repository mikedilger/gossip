use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use nostr_types::Id;
use std::env;

pub fn handle_command(mut args: env::Args) -> Result<(), Error> {
    let _ = args.next(); // program name
    let command = args.next().unwrap(); // must be there or we would not have been called

    println!("\n*** Gossip is running in command mode ***");
    println!("*** COMMAND = {} ***\n", command);

    match &*command {
        "test" => println!("Test successful"),
        "dump_event" => dump_event(args)?,
        other => println!("Unknown command {}", other),
    }

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
        None => println!("Event not found"),
    }

    Ok(())
}

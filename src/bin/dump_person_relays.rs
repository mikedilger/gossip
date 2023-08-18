use heed::types::UnalignedSlice;
use heed::{EnvFlags, EnvOpenOptions};
use nostr_types::{PublicKey, RelayUrl};
use speedy::{Readable, Writable};
use std::path::PathBuf;
use std::{env, fmt};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args();
    let _ = args.next(); // program name
    let pubkeyhex = match args.next() {
        Some(data) => data,
        None => panic!("Usage: dump_person_relays <PublicKeyHex>"),
    };
    let pubkey = PublicKey::try_from_hex_string(&pubkeyhex, true).unwrap();

    let mut builder = EnvOpenOptions::new();
    unsafe {
        builder.flags(EnvFlags::NO_SYNC);
    }
    builder.max_dbs(32);
    builder.map_size(1048576 * 1024 * 24); // 24 GB
    let pathbuf = PathBuf::from("/home/mike/.local/share/gossip/unstable/lmdb");
    let env = builder.open(pathbuf.as_path())?;

    let mut txn = env.write_txn()?;
    let person_relays = env
        .database_options()
        .types::<UnalignedSlice<u8>, UnalignedSlice<u8>>()
        .name("person_relays")
        .create(&mut txn)?;
    txn.commit()?;

    let start_key = pubkey.to_bytes();
    let txn = env.read_txn()?;
    let iter = person_relays.prefix_iter(&txn, &start_key)?;
    let mut output: Vec<PersonRelay> = Vec::new();
    for result in iter {
        let (_key, val) = result?;
        let person_relay = PersonRelay::read_from_buffer(val)?;
        output.push(person_relay);
    }

    for pr in &output {
        println!("{}", pr);
    }

    Ok(())
}

#[derive(Debug, Readable, Writable)]
pub struct PersonRelay {
    // The person
    pub pubkey: PublicKey,

    // The relay associated with that person
    pub url: RelayUrl,

    // The last time we fetched one of the person's events from this relay
    pub last_fetched: Option<u64>,

    // When we follow someone at a relay
    pub last_suggested_kind3: Option<u64>,

    // When we get their nip05 and it specifies this relay
    pub last_suggested_nip05: Option<u64>,

    // Updated when a 'p' tag on any event associates this person and relay via the
    // recommended_relay_url field
    pub last_suggested_bytag: Option<u64>,

    pub read: bool,

    pub write: bool,

    // When we follow someone at a relay, this is set true
    pub manually_paired_read: bool,

    // When we follow someone at a relay, this is set true
    pub manually_paired_write: bool,
}

impl fmt::Display for PersonRelay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "{{ pubkey: {}, write: {}, read: {}, url: {}, last_fetched: {:?}, last_suggested_kind3: {:?}, last_suggested_nip05: {:?}, last_suggested_bytag: {:?}, manually_paired_read: {}, manually_paired_write: {} }}",
            self.pubkey.as_hex_string(),
            self.write,
            self.read,
            self.url,
            self.last_fetched,
            self.last_suggested_kind3,
            self.last_suggested_nip05,
            self.last_suggested_bytag,
            self.manually_paired_read,
            self.manually_paired_write
        )
    }
}

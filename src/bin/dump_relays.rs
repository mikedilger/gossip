use heed::types::UnalignedSlice;
use heed::{EnvFlags, EnvOpenOptions};
use nostr_types::{RelayInformationDocument, RelayUrl};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
//use std::{env, fmt};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut builder = EnvOpenOptions::new();
    unsafe {
        builder.flags(EnvFlags::NO_SYNC);
    }
    builder.max_dbs(32);
    builder.map_size(1048576 * 1024 * 24); // 24 GB
    let pathbuf = PathBuf::from("/home/mike/.local/share/gossip/unstable/lmdb");
    let env = builder.open(pathbuf.as_path())?;

    let mut txn = env.write_txn()?;
    let relays = env
        .database_options()
        .types::<UnalignedSlice<u8>, UnalignedSlice<u8>>()
        .name("relays")
        .create(&mut txn)?;
    txn.commit()?;

    let txn = env.read_txn()?;
    let iter = relays.iter(&txn)?;
    let mut output: Vec<Relay> = Vec::new();
    for result in iter {
        let (_key, val) = result?;
        let relay: Relay = serde_json::from_slice(val)?;
        output.push(relay);
    }

    for relay in &output {
        if relay.usage_bits & 1 << 2 != 0 {
            println!("ADV: {}", relay.url);
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relay {
    pub url: RelayUrl,
    pub success_count: u64,
    pub failure_count: u64,
    pub last_connected_at: Option<u64>,
    pub last_general_eose_at: Option<u64>,
    pub rank: u64,
    pub hidden: bool,
    pub usage_bits: u64,
    pub nip11: Option<RelayInformationDocument>,
    pub last_attempt_nip11: Option<u64>,
}

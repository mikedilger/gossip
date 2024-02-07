use crate::error::Error;
use crate::nip46::{Approval, Nip46ClientMetadata, Nip46Server};
use crate::storage::Storage;
use heed::RwTxn;
use nostr_types::{PublicKey, RelayUrl};
use speedy::{Readable, Writable};

#[derive(Debug, Clone, Readable, Writable)]
pub struct Nip46Server1 {
    pub peer_pubkey: PublicKey,
    pub relays: Vec<RelayUrl>,
    pub metadata: Option<Nip46ClientMetadata>,
}

impl Storage {
    pub(super) fn m26_trigger(&self) -> Result<(), Error> {
        let _ = self.db_nip46servers1()?;
        let _ = self.db_nip46servers2()?;
        Ok(())
    }

    pub(super) fn m26_migrate<'a>(
        &'a self,
        prefix: &str,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: migrating nostr connect services...");

        // Migrate
        self.m26_migrate_nostr_connect_services(txn)?;

        Ok(())
    }

    fn m26_migrate_nostr_connect_services<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        let loop_txn = self.env.read_txn()?;
        for result in self.db_nip46servers1()?.iter(&loop_txn)? {
            let (key, val) = result?;
            let server1 = Nip46Server1::read_from_buffer(val)?;
            let server2 = Nip46Server {
                peer_pubkey: server1.peer_pubkey,
                relays: server1.relays,
                metadata: server1.metadata,
                sign_approval: Approval::None,
                encrypt_approval: Approval::None,
                decrypt_approval: Approval::None,
            };
            let bytes = server2.write_to_vec()?;
            self.db_nip46servers2()?.put(txn, key, &bytes)?;
        }

        // clear old database (we don't have an interface to delete it)
        self.db_nip46servers1()?.clear(txn)?;

        Ok(())
    }
}

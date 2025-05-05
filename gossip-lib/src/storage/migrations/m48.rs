use crate::error::Error;
use crate::storage::Storage;
use heed::RwTxn;
use nostr_types::{EncryptedPrivateKey, Identity, PublicKey};
use speedy::Readable;

impl Storage {
    pub(super) fn m48_trigger(&self) -> Result<(), Error> {
        Ok(())
    }

    pub(super) fn m48_migrate<'a>(
        &'a self,
        prefix: &str,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: Migrating identity...");

        // Identity
        let opt_pk: Option<PublicKey> = match self.db_general().unwrap().get(txn, b"public_key") {
            Err(_) => None,
            Ok(None) => None,
            Ok(Some(bytes)) => Option::<PublicKey>::read_from_buffer(bytes).unwrap_or_default(),
        };

        let opt_epk_result: Result<Option<EncryptedPrivateKey>, Error> =
            match self.db_general()?.get(txn, b"encrypted_private_key")? {
                None => Ok(None),
                Some(bytes) => {
                    let os = Option::<String>::read_from_buffer(bytes)?;
                    Ok(os.map(EncryptedPrivateKey))
                }
            };
        let opt_epk = opt_epk_result?;

        let identity = match (opt_pk, opt_epk) {
            (Some(pk), Some(epk)) => Identity::from_locked_parts(pk, epk),
            (Some(pk), None) => Identity::from_public_key(pk),
            (None, _) => Identity::None,
        };

        self.write_identity(&identity, Some(txn))?;

        // Client Identity

        let opt_pk: Option<PublicKey> =
            match self.db_general().unwrap().get(txn, b"client_public_key") {
                Err(_) => None,
                Ok(None) => None,
                Ok(Some(bytes)) => Option::<PublicKey>::read_from_buffer(bytes).unwrap_or_default(),
            };

        let opt_epk_result: Result<Option<EncryptedPrivateKey>, Error> = match self
            .db_general()?
            .get(txn, b"client_encrypted_private_key")?
        {
            None => Ok(None),
            Some(bytes) => {
                let os = Option::<String>::read_from_buffer(bytes)?;
                Ok(os.map(EncryptedPrivateKey))
            }
        };
        let opt_epk = opt_epk_result?;

        let client_identity = match (opt_pk, opt_epk) {
            (Some(pk), Some(epk)) => Identity::from_locked_parts(pk, epk),
            (Some(pk), None) => Identity::from_public_key(pk),
            (None, _) => Identity::None,
        };

        self.write_client_identity(&client_identity, Some(txn))?;

        Ok(())
    }
}


impl Signer {
    // CONSUMING ...


    pub fn sign_preevent(
        &self,
        preevent: PreEvent,
        pow: Option<u8>,
        work_sender: Option<Sender<u8>>,
    ) -> Result<Event, Error> {
        unimplemented!()
        /*
            match &*self.private.read() {
                Some(pk) => match pow {
                    Some(pow) => Ok(Event::new_with_pow(preevent, pk, pow, work_sender)?),
                    None => Ok(Event::new(preevent, pk)?),
                },
                _ => Err((ErrorKind::NoPrivateKey, file!(), line!()).into()),
        }
                */
    }

    /// Export the private key as bech32 (decrypted!)
    pub fn export_private_key_bech32(&self, pass: &str) -> Result<String, Error> {
        let maybe_encrypted = self.encrypted.read().to_owned();
        match maybe_encrypted {
            Some(epk) => {
                // Test password
                let mut pk = epk.decrypt(pass)?;

                let output = pk.as_bech32_string();

                // We have to regenerate encrypted private key because it may have fallen from
                // medium to weak security. And then we need to save that
                let epk = pk.export_encrypted(pass, GLOBALS.storage.read_setting_log_n())?;
                *self.encrypted.write() = Some(epk);
                *self.private.write() = Some(pk);
                self.save()?;
                Ok(output)
            }
            _ => Err((ErrorKind::NoPrivateKey, file!(), line!()).into()),
        }
    }

    /// Export the private key as hex (decrypted!)
    pub fn export_private_key_hex(&self, pass: &str) -> Result<String, Error> {
        let maybe_encrypted = self.encrypted.read().to_owned();
        match maybe_encrypted {
            Some(epk) => {
                // Test password
                let mut pk = epk.decrypt(pass)?;

                let output = pk.as_hex_string();

                // We have to regenerate encrypted private key because it may have fallen from
                // medium to weak security. And then we need to save that
                let epk = pk.export_encrypted(pass, GLOBALS.storage.read_setting_log_n())?;
                *self.encrypted.write() = Some(epk);
                *self.private.write() = Some(pk);
                self.save()?;
                Ok(output)
            }
            _ => Err((ErrorKind::NoPrivateKey, file!(), line!()).into()),
        }
    }

    pub(crate) fn delete_identity(&self) {
        *self.private.write() = None;
        *self.encrypted.write() = None;
        *self.public.write() = None;
        let _ = self.save();
    }

    /// Decrypt an event
    pub fn decrypt_message(&self, event: &Event) -> Result<String, Error> {
        unimplemented!()
        /*
            match &*self.private.read() {
                Some(private) => Ok(event.decrypted_contents(private)?),
                _ => Err((ErrorKind::NoPrivateKey, file!(), line!()).into()),
        }
                */
    }

    /// Unwrap a giftwrap event
    pub fn unwrap_giftwrap(&self, event: &Event) -> Result<Rumor, Error> {
        unimplemented!()
        /*
            match &*self.private.read() {
                Some(private) => Ok(event.giftwrap_unwrap(private)?),
                _ => Err((ErrorKind::NoPrivateKey, file!(), line!()).into()),
        }
                */
    }

    /// Unwrap a giftwrap event V1
    pub fn unwrap_giftwrap1(&self, event: &EventV1) -> Result<RumorV1, Error> {
        unimplemented!()
        /*
            match &*self.private.read() {
                Some(private) => Ok(event.giftwrap_unwrap(private)?),
                _ => Err((ErrorKind::NoPrivateKey, file!(), line!()).into()),
        }
                */
    }

    /// Encrypt content
    pub fn encrypt(
        &self,
        other: &PublicKey,
        plaintext: &str,
        algo: ContentEncryptionAlgorithm,
    ) -> Result<String, Error> {
        match &*self.private.read() {
            Some(private) => Ok(private.encrypt(other, plaintext, algo)?),
            None => Err((ErrorKind::NoPrivateKey, file!(), line!()).into()),
        }
    }

    /// Decrypt NIP-04 content
    pub fn decrypt_nip04(&self, other: &PublicKey, ciphertext: &str) -> Result<Vec<u8>, Error> {
        match &*self.private.read() {
            Some(private) => Ok(private.decrypt_nip04(other, ciphertext)?),
            None => Err((ErrorKind::NoPrivateKey, file!(), line!()).into()),
        }
    }

    /// Decrypt NIP-44 content
    pub fn decrypt_nip44(&self, other: &PublicKey, ciphertext: &str) -> Result<String, Error> {
        match &*self.private.read() {
            Some(private) => Ok(private.decrypt_nip44(other, ciphertext)?),
            None => Err((ErrorKind::NoPrivateKey, file!(), line!()).into()),
        }
    }
}

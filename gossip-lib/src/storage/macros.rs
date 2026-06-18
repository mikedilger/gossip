macro_rules! key {
    ($slice:expr) => {
        if $slice.len() > 511 {
            &$slice[..=510]
        } else {
            $slice
        }
    };
}

macro_rules! maybe_local_txn {
    ($storage:expr, $rw_txn:ident, $local_txn: ident) => {
       match $rw_txn {
            Some(x) => x,
            None => {
                $local_txn = Some($storage.get_write_txn()?);
                $local_txn.as_mut().unwrap()
            }
        }
    };
}

macro_rules! maybe_local_txn_commit {
    ($local_txn: ident) => {
        if let Some(txn) = $local_txn {
            txn.commit()?;
        }
    }
}

// Macro to define read-and-write into "general" database, largely for settings
// The type must implemented Speedy Readable and Writable
macro_rules! def_setting {
    ($field:ident, $string:literal, $type:ty, $default:expr) => {
        paste! {
            #[allow(dead_code)]
            pub fn [<write_setting_ $field>]<'a>(
                &'a self,
                $field: &$type,
                rw_txn: Option<&mut RwTxn<'a>>,
            ) -> Result<(), Error> {
                let bytes = $field.write_to_vec()?;

                let mut local_txn = None;
                let txn = maybe_local_txn!(self, rw_txn, local_txn);

                let rval = self.db_general()?.put(txn, $string, &bytes)?;

                maybe_local_txn_commit!(local_txn);

                Ok(rval)
            }

            #[allow(dead_code)]
            pub fn [<read_setting_ $field>](&self) -> $type {
                let txn = match self.env.read_txn() {
                    Ok(txn) => txn,
                    Err(_) => return $default,
                };

                match self.db_general().unwrap().get(&txn, $string) {
                    Err(_) => $default,
                    Ok(None) => $default,
                    Ok(Some(bytes)) => match <$type>::read_from_buffer(bytes) {
                        Ok(val) => val,
                        Err(_) => $default,
                    }
                }
            }

            #[allow(dead_code)]
            pub(crate) fn [<set_default_setting_ $field>]<'a>(
                &'a self,
                rw_txn: Option<&mut RwTxn<'a>>
            ) -> Result<(), Error> {
                self.[<write_setting_ $field>](&$default, rw_txn)
            }

            #[allow(dead_code)]
            pub fn [<get_default_setting_ $field>]() -> $type {
                $default
            }
        }
    };
}

macro_rules! def_flag {
    ($field:ident, $string:literal, $default:expr) => {
        paste! {
            pub fn [<set_flag_ $field>]<'a>(
                &'a self,
                $field: bool,
                rw_txn: Option<&mut RwTxn<'a>>,
            ) -> Result<(), Error> {
                let bytes = $field.write_to_vec()?;

                let mut local_txn = None;
                let txn = maybe_local_txn!(self, rw_txn, local_txn);

                let rval = self.db_general()?.put(txn, $string, &bytes)?;

                maybe_local_txn_commit!(local_txn);

                Ok(rval)
            }

            pub fn [<get_flag_ $field>](&self) -> bool {
                let txn = match self.env.read_txn() {
                    Ok(txn) => txn,
                    Err(_) => return $default,
                };

                match self.db_general().unwrap().get(&txn, $string) {
                    Err(_) => $default,
                    Ok(None) => $default,
                    Ok(Some(bytes)) => bool::read_from_buffer(bytes).unwrap_or($default),
                }
            }
        }
    };
}


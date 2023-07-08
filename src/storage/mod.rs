use crate::error::Error;

pub struct Storage {}

impl Storage {
    pub fn new() -> Result<Storage, Error> {
        Ok(Storage {})
    }
}

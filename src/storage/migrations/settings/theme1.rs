use serde::{Deserialize, Serialize};
use speedy::{Readable, Writable};

// note: if we store anything inside the variants, we can't use macro_rules.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize, Readable, Writable)]
pub enum ThemeVariant1 {
    Classic,
    Default,
    Roundy,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize, Readable, Writable)]
pub struct Theme1 {
    pub variant: ThemeVariant1,
    pub dark_mode: bool,
    pub follow_os_dark_mode: bool,
}

use crate::db::DbRelay;
use crate::relay_picker::RelayAssignment;

/// All the per-relay information we need kept hot in memory
#[derive(Debug, Clone)]
pub struct RelayInfo {
    pub dbrelay: DbRelay,
    pub connected: bool,
    pub assignment: Option<RelayAssignment>,
    //pub subscriptions: Vec<String>,
}

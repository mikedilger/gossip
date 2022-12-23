/// A relationship between events
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Relationship {
    Reply,
    Quote,
    Reaction(String),
    Deletion(String),
}

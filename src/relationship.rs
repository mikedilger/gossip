/// A relationship between events
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Relationship {
    Reply,
    Mention,
    Reaction(String),
    Deletion(String),
}

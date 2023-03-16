/// A relationship between events
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Relationship {
    Root,
    Reply,
    Mention,
    Reaction(String),
    Deletion(String),
}

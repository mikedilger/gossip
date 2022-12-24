/// A relationship between events
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Relationship {
    Reply,
    #[allow(dead_code)]
    Quote,
    Reaction(String),
    Deletion(String),
}

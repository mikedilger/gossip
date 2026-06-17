use crate::Id;

/// The state of authentication to the relay
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum AuthState {
    /// AUTH has not been requested by the relay
    #[default]
    NotYetRequested,

    /// AUTH has been requested
    Challenged(String),

    /// AUTH is in progress, we have sent the event
    InProgress(Id),

    /// AUTH succeeded
    Success,

    /// AUTH failed
    Failure(String),
}

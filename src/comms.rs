use nostr_types::{Event, Id, IdHex, PublicKey, PublicKeyHex, Tag};

/// This is a message sent to the Overlord
#[derive(Debug, Clone)]
pub enum ToOverlordMessage {
    AddRelay(String),
    DeletePub,
    FollowBech32(String, String),
    FollowHex(String, String),
    FollowNip05(String),
    GeneratePrivateKey(String),
    ImportPriv(String, String),
    ImportPub(String),
    Like(Id, PublicKey),
    MinionIsReady,
    ProcessIncomingEvents,
    PostReply(String, Vec<Tag>, Id),
    PostTextNote(String, Vec<Tag>),
    PruneDatabase,
    PullFollowMerge,
    PullFollowOverwrite,
    PushFollow,
    SaveRelays,
    SaveSettings,
    SetThreadFeed(Id),
    Shutdown,
    UnlockKey(String),
    UpdateMetadata(PublicKeyHex),
}

/// This is a message sent to the minions
#[derive(Debug, Clone)]
pub struct ToMinionMessage {
    /// The minion we are addressing, based on the URL they are listening to
    /// as a String.  "all" means all minions.
    pub target: String,

    pub payload: ToMinionPayload,
}

#[derive(Debug, Clone)]
pub enum ToMinionPayload {
    FetchEvents(Vec<IdHex>),
    PostEvent(Box<Event>),
    PullFollowing,
    Shutdown,
    SubscribeGeneralFeed,
    SubscribePersonFeed(PublicKeyHex),
    SubscribeThreadFeed(IdHex, Vec<IdHex>),
    TempSubscribeMetadata(PublicKeyHex),
    UnsubscribeThreadFeed,
}

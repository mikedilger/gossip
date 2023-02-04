use nostr_types::{Event, Id, IdHex, Metadata, PublicKey, PublicKeyHex, RelayUrl, Tag};

/// This is a message sent to the Overlord
#[derive(Debug, Clone)]
pub enum ToOverlordMessage {
    AddRelay(RelayUrl),
    DeletePub,
    DeletePriv(String),
    FollowPubkeyAndRelay(String, RelayUrl),
    FollowNip05(String),
    FollowNprofile(String),
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
    PushMetadata(Metadata),
    RefreshFollowedMetadata,
    RankRelay(RelayUrl, u8),
    SaveSettings,
    SetRelayPost(RelayUrl, bool),
    SetThreadFeed(Id, Id, Option<Id>),
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
    SubscribeGeneralFeed(Vec<PublicKeyHex>),
    SubscribePersonFeed(PublicKeyHex),
    SubscribeThreadFeed(IdHex, Vec<IdHex>),
    TempSubscribeMetadata(Vec<PublicKeyHex>),
    UnsubscribeThreadFeed,
}

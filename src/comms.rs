use nostr_types::{Event, Id, IdHex, Metadata, PublicKey, PublicKeyHex, RelayUrl, Tag};

/// This is a message sent to the Overlord
#[derive(Debug, Clone)]
pub enum ToOverlordMessage {
    AddRelay(RelayUrl),
    AdvertiseRelayList,
    ChangePassphrase(String, String),
    DeletePub,
    DeletePriv(String),
    DropRelay(RelayUrl),
    FollowPubkeyAndRelay(String, RelayUrl),
    FollowNip05(String),
    FollowNprofile(String),
    GeneratePrivateKey(String),
    ImportPriv(String, String),
    ImportPub(String),
    Like(Id, PublicKey),
    MinionIsReady,
    PickRelays,
    ProcessIncomingEvents,
    Post(String, Vec<Tag>, Option<Id>),
    PruneDatabase,
    PullFollowMerge,
    PullFollowOverwrite,
    PushFollow,
    PushMetadata(Metadata),
    RefreshFollowedMetadata,
    RankRelay(RelayUrl, u8),
    SaveSettings,
    SetActivePerson(PublicKeyHex),
    SetRelayReadWrite(RelayUrl, bool, bool),
    SetRelayAdvertise(RelayUrl, bool),
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
    SubscribeMentions,
    SubscribeConfig,
    SubscribePersonFeed(PublicKeyHex),
    SubscribeThreadFeed(IdHex, Vec<IdHex>),
    TempSubscribeMetadata(Vec<PublicKeyHex>),
    UnsubscribePersonFeed,
    UnsubscribeThreadFeed,
}

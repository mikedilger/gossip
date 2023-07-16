use nostr_types::{
    Event, Id, IdHex, Metadata, MilliSatoshi, PublicKey, PublicKeyHex, RelayUrl, Tag, UncheckedUrl,
};

/// This is a message sent to the Overlord
#[derive(Debug, Clone)]
pub enum ToOverlordMessage {
    AddRelay(RelayUrl),
    AdvertiseRelayList,
    ChangePassphrase(String, String),
    ClearFollowing,
    DelegationReset,
    DeletePost(Id),
    DeletePriv,
    DeletePub,
    DropRelay(RelayUrl),
    FetchEvent(Id, Vec<RelayUrl>),
    FollowAuto(String, Option<RelayUrl>),
    GeneratePrivateKey(String),
    HideOrShowRelay(RelayUrl, bool),
    ImportPriv(String, String),
    ImportPub(String),
    Like(Id, PublicKey),
    MinionIsReady,
    MinionJobComplete(RelayUrl, u64),
    MinionJobUpdated(RelayUrl, u64, u64),
    PickRelays,
    ProcessIncomingEvents,
    Post(String, Vec<Tag>, Option<Id>),
    PruneDatabase,
    PullFollow,
    PushFollow,
    PushMetadata(Metadata),
    ReengageMinion(RelayUrl, Vec<RelayJob>),
    RefreshFollowedMetadata,
    Repost(Id),
    RankRelay(RelayUrl, u8),
    SaveSettings,
    Search(String),
    SetActivePerson(PublicKeyHex),
    AdjustRelayUsageBit(RelayUrl, u64, bool),
    SetThreadFeed(Id, Id, Vec<RelayUrl>, Option<PublicKeyHex>),
    Shutdown,
    UnlockKey(String),
    UpdateFollowing(bool),
    UpdateMetadata(PublicKeyHex),
    UpdateMetadataInBulk(Vec<PublicKeyHex>),
    VisibleNotesChanged(Vec<Id>),
    ZapStart(Id, PublicKey, UncheckedUrl),
    Zap(Id, PublicKey, MilliSatoshi, String),
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
pub struct ToMinionPayload {
    /// A job id, so the minion and overlord can talk about the job.
    pub job_id: u64,

    pub detail: ToMinionPayloadDetail,
}

#[derive(Debug, Clone)]
pub enum ToMinionPayloadDetail {
    FetchEvent(IdHex),
    PostEvent(Box<Event>),
    PullFollowing,
    Shutdown,
    SubscribeAugments(Vec<IdHex>),
    SubscribeConfig,
    SubscribeDiscover(Vec<PublicKeyHex>),
    SubscribeGeneralFeed(Vec<PublicKeyHex>),
    SubscribeMentions,
    SubscribePersonFeed(PublicKeyHex),
    SubscribeThreadFeed(IdHex, Vec<IdHex>),
    TempSubscribeMetadata(Vec<PublicKeyHex>),
    UnsubscribePersonFeed,
    UnsubscribeThreadFeed,
}

#[derive(Debug, Clone)]
pub struct RelayJob {
    // Short reason for human viewing
    pub reason: &'static str,

    // Payload sent when it was started
    pub payload: ToMinionPayload,

    // Persistent? (restart if we get disconnected)
    pub persistent: bool,
    // NOTE, there is other per-relay data stored elsewhere in
    //   overlord.minions_task_url
    //   GLOBALS.relay_picker
}

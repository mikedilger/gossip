use nostr_types::{
    Event, Id, IdHex, Metadata, MilliSatoshi, PublicKey, RelayUrl, Tag, UncheckedUrl,
};
use std::fmt;

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
    FollowPubkeyAndRelay(String, RelayUrl),
    FollowNip05(String),
    FollowNprofile(String),
    GeneratePrivateKey(String),
    HideOrShowRelay(RelayUrl, bool),
    ImportPriv(String, String),
    ImportPub(String),
    Like(Id, PublicKey),
    MinionIsReady,
    MinionJobComplete(RelayUrl, u64),
    MinionJobUpdated(RelayUrl, u64, u64),
    PickRelays,
    Post(String, Vec<Tag>, Option<Id>),
    PruneDatabase,
    PullFollow,
    PushFollow,
    PushMetadata(Metadata),
    ReengageMinion(RelayUrl, Vec<RelayJob>),
    RefreshFollowedMetadata,
    ClearAllUsageOnRelay(RelayUrl),
    Repost(Id),
    RankRelay(RelayUrl, u8),
    Search(String),
    SetActivePerson(PublicKey),
    AdjustRelayUsageBit(RelayUrl, u64, bool),
    SetThreadFeed(Id, Id, Vec<RelayUrl>, Option<PublicKey>),
    Shutdown,
    UnlockKey(String),
    UpdateFollowing(bool),
    UpdateMetadata(PublicKey),
    UpdateMetadataInBulk(Vec<PublicKey>),
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
    SubscribeDiscover(Vec<PublicKey>),
    SubscribeGeneralFeed(Vec<PublicKey>),
    SubscribeMentions,
    SubscribePersonFeed(PublicKey),
    SubscribeThreadFeed(IdHex, Vec<IdHex>),
    TempSubscribeMetadata(Vec<PublicKey>),
    UnsubscribePersonFeed,
    UnsubscribeThreadFeed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayConnectionReason {
    Advertising,
    Config,
    Discovery,
    FetchAugments,
    FetchContacts,
    FetchEvent,
    FetchMentions,
    FetchMetadata,
    Follow,
    PostEvent,
    PostContacts,
    PostLike,
    PostMetadata,
    ReadThread,
}

impl fmt::Display for RelayConnectionReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)?;
        Ok(())
    }
}

impl RelayConnectionReason {
    pub fn description(&self) -> &'static str {
        use RelayConnectionReason::*;
        match *self {
            Discovery => "Searching for other people's Relay Lists",
            Config => "Reading our client configuration",
            FetchMentions => "Searching for mentions of us",
            Follow => "Following the posts of people in our Contact List",
            FetchAugments => "Fetching events that augment other events (likes, zaps, deletions)",
            FetchEvent => "Fetching a particular event",
            FetchMetadata => "Fetching metadata for a person",
            PostEvent => "Posting an event",
            Advertising => "Advertising our relay list",
            PostLike => "Posting a reaction to an event",
            FetchContacts => "Fetching our contact list",
            PostContacts => "Posting our contact list",
            PostMetadata => "Posting our metadata",
            ReadThread => "Reading ancestors to build a thread",
        }
    }

    pub fn persistent(&self) -> bool {
        use RelayConnectionReason::*;
        match *self {
            Discovery => false,
            Config => false,
            FetchMentions => true,
            Follow => true,
            FetchAugments => false,
            FetchEvent => false,
            FetchMetadata => false,
            PostEvent => false,
            Advertising => false,
            PostLike => false,
            FetchContacts => false,
            PostContacts => false,
            PostMetadata => false,
            ReadThread => true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RelayJob {
    // Short reason for human viewing
    pub reason: RelayConnectionReason,

    // Payload sent when it was started
    pub payload: ToMinionPayload,
    // NOTE, there is other per-relay data stored elsewhere in
    //   overlord.minions_task_url
    //   GLOBALS.relay_picker
}

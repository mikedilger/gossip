use crate::dm_channel::DmChannel;
use crate::filter_set::FilterSet;
use crate::misc::Private;
use crate::nip46::{Approval, ParsedCommand};
use crate::people::PersonList;
use crate::relay::Relay;
use nostr_types::{
    Event, EventReference, Id, IdHex, Metadata, MilliSatoshi, NAddr, Profile, PublicKey, RelayUrl,
    Tag, UncheckedUrl, Unixtime,
};
use std::fmt;
use std::hash::{Hash, Hasher};

/// This is a message sent to the Overlord. Tasks which take any amount of time,
/// especially involving relays, are handled by the Overlord in this way. There is
/// no return value, you'll have to check various GLOBALS state later on if you
/// depend on the result. Such an architecture works best with an immediate-mode
/// renderer.
#[derive(Debug, Clone)]
pub enum ToOverlordMessage {
    /// Calls [add_relay](crate::Overlord::add_relay)
    AddRelay(RelayUrl),

    /// Calls [advertise_relay_list](crate::Overlord::advertise_relay_list)
    AdvertiseRelayList,

    /// Calls [advertise_relay_list_one](crate::Overlord::advertise_relay_list)
    AdvertiseRelayListOne(RelayUrl, Box<Event>, Box<Event>),

    /// Calls [auth_approved](crate::Overlord::auth_approved)
    /// pass 'true' as the second parameter for a permanent approval
    AuthApproved(RelayUrl, bool),

    /// Calls [auth_approved](crate::Overlord::auth_declined)
    /// pass 'true' as the second parameter for a permanent approval
    AuthDeclined(RelayUrl, bool),

    /// Calls [bookmark_add](crate::Overlord::bookmark_add)
    /// Adds a bookmark, possibly privately, and publishes new bookmarks list
    BookmarkAdd(EventReference, bool),

    /// Calls [bookmark_rm](crate::Overlord::bookmark_rm)
    /// Removess a bookmark, and publishes new bookmarks list
    BookmarkRm(EventReference),

    /// Calls [change_passphrase](crate::Overlord::change_passphrase)
    ChangePassphrase { old: String, new: String },

    /// Calls [clear_person_list](crate::Overlord::clear_person_list)
    ClearPersonList(PersonList),

    /// Calls [auth_approved](crate::Overlord::connect_approved)
    /// pass 'true' as the second parameter for a permanent approval
    ConnectApproved(RelayUrl, bool),

    /// Calls [auth_approved](crate::Overlord::connect_declined)
    /// pass 'true' as the second parameter for a permanent approval
    ConnectDeclined(RelayUrl, bool),

    /// Calls [delegation_reset](crate::Overlord::delegation_reset)
    DelegationReset,

    /// Calls [delete_person_list](crate::Overlord::delete_person_list)
    DeletePersonList(PersonList),

    /// Calls [delete_post](crate::Overlord::delete_post)
    DeletePost(Id),

    /// Calls [delete_priv](crate::Overlord::delete_priv)
    DeletePriv,

    /// Calls [delete_pub](crate::Overlord::delete_pub)
    DeletePub,

    /// Calls [drop_relay](crate::Overlord::drop_relay)
    DropRelay(RelayUrl),

    /// Calls [fetch_event](crate::Overlord::fetch_event)
    FetchEvent(Id, Vec<RelayUrl>),

    /// Calls [fetch_naddr](crate::Overlord::fetch_naddr)
    FetchNAddr(NAddr),

    /// Calls [follow_pubkey](crate::Overlord::follow_pubkey)
    FollowPubkey(PublicKey, PersonList, Private),

    /// Calls [follow_nip05](crate::Overlord::follow_nip05)
    FollowNip05(String, PersonList, Private),

    /// Calls [follow_nprofile](crate::Overlord::follow_nprofile)
    FollowNprofile(Profile, PersonList, Private),

    /// Calls [generate_private_key](crate::Overlord::generate_private_key)
    GeneratePrivateKey(String),

    /// Calls [hide_or_show_relay](crate::Overlord::hide_or_show_relay)
    HideOrShowRelay(RelayUrl, bool),

    /// Calls [import_priv](crate::Overlord::import_priv)
    ImportPriv {
        // nsec, hex, or ncryptsec
        privkey: String,
        password: String,
    },

    /// Calls [import_pub](crate::Overlord::import_pub)
    ImportPub(String),

    /// Calls [load_more_current_feed](crate::Overlord::load_more_current_feed)
    LoadMoreCurrentFeed,

    /// internal (minions use this channel too)
    MinionJobComplete(RelayUrl, u64),

    /// internal (minions use this channel too)
    MinionJobUpdated(RelayUrl, u64, u64),

    /// Calls [nip46_server_op_approval_response](crate::Overlord::nip46_server_op_approval_response)
    Nip46ServerOpApprovalResponse(PublicKey, ParsedCommand, Approval),

    /// Calls [post](crate::Overlord::post)
    Post {
        content: String,
        tags: Vec<Tag>,
        in_reply_to: Option<Id>,
        annotation: bool,
        dm_channel: Option<DmChannel>,
    },

    /// Calls [post_again](crate::Overlord::post_again)
    PostAgain(Event),

    /// Calls [post_nip46_event](crate::Overlord::post_nip46_event)
    PostNip46Event(Event, Vec<RelayUrl>),

    /// Calls [prune_cache](crate::Overlord::prune_cache)
    PruneCache,

    /// Calls [prune_database](crate::Overlord::prune_database)
    PruneDatabase,

    /// Calls [push_person_list](crate::Overlord::push_person_list)
    PushPersonList(PersonList),

    /// Calls [push_metadata](crate::Overlord::push_metadata)
    PushMetadata(Metadata),

    /// Calls [rank_relay](crate::Overlord::rank_relay)
    RankRelay(RelayUrl, u8),

    /// Calls [react](crate::Overlord::react)
    React(Id, PublicKey, char),

    /// internal (the overlord sends messages to itself sometimes!)
    ReengageMinion(RelayUrl, Vec<RelayJob>),

    /// Calls [refresh_scores_and_pick_relays](crate::Overlord::refresh_scores_and_pick_relays)
    RefreshScoresAndPickRelays,

    /// Calls [reresh_subscribed_metadata](crate::Overlord::refresh_subscribed_metadata)
    RefreshSubscribedMetadata,

    /// Calls [repost](crate::Overlord::repost)
    Repost(Id),

    /// Calls [search](crate::Overlord::search)
    Search(String),

    /// Calls [set_active_person](crate::Overlord::set_active_person)
    SetActivePerson(PublicKey),

    /// internal
    SetDmChannel(DmChannel),

    /// internal
    SetGlobalFeed(Unixtime),

    /// internal
    SetPersonFeed(PublicKey, Unixtime),

    /// internal
    SetThreadFeed {
        id: Id,
        referenced_by: Id,
        author: Option<PublicKey>,
    },

    /// Calls [start_long_lived_subscriptions](crate::Overlord::start_long_lived_subscriptions)
    StartLongLivedSubscriptions,

    /// Calls [subscribe_config](crate::Overlord::subscribe_config)
    SubscribeConfig(Option<Vec<RelayUrl>>),

    /// Calls [subscribe_discover](crate::Overlord::subscribe_discover)
    SubscribeDiscover(Vec<PublicKey>, Option<Vec<RelayUrl>>),

    /// Calls [subscribe_inbox](crate::Overlord::subscribe_inbox)
    SubscribeInbox(Option<Vec<RelayUrl>>),

    /// Calls [subscribe_nip46](crate::Overlord::subscribe_nip46)
    SubscribeNip46(Vec<RelayUrl>),

    /// Calls [unlock_key](crate::Overlord::unlock_key)
    UnlockKey(String),

    /// Calls [update_metadata](crate::Overlord::update_metadata)
    UpdateMetadata(PublicKey),

    /// Calls [update_metadata_in_bulk](crate::Overlord::update_metadata_in_bulk)
    UpdateMetadataInBulk(Vec<PublicKey>),

    /// Calls [update_person_list](crate::Overlord::update_person_list)
    UpdatePersonList {
        person_list: PersonList,
        merge: bool,
    },

    /// Calls [update_relay](crate::Overlord::update_relay)
    UpdateRelay(Relay, Relay),

    /// Calls [visible_notes_changed](crate::Overlord::visible_notes_changed)
    VisibleNotesChanged(Vec<Id>),

    /// Calls [zap_start](crate::Overlord::zap_start)
    ZapStart(Id, PublicKey, UncheckedUrl),

    /// Calls [zap](crate::Overlord::zap)
    Zap(Id, PublicKey, MilliSatoshi, String),
}

/// Internal to gossip-lib.
/// This is a message sent to the minions
#[derive(Debug, Clone)]
pub(crate) struct ToMinionMessage {
    /// The minion we are addressing, based on the URL they are listening to
    /// as a String.  "all" means all minions.
    pub target: String,

    pub payload: ToMinionPayload,
}

#[derive(Debug, Clone)]
pub(crate) struct ToMinionPayload {
    /// A job id, so the minion and overlord can talk about the job.
    /// If set to 0, the job is not restarted on failure.
    pub job_id: u64,

    pub detail: ToMinionPayloadDetail,
}

impl PartialEq for ToMinionPayload {
    fn eq(&self, other: &Self) -> bool {
        self.detail == other.detail
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ToMinionPayloadDetail {
    AdvertiseRelayList(Box<Event>, Box<Event>),
    AuthApproved,
    AuthDeclined,
    FetchEvent(Id),
    FetchNAddr(NAddr),
    PostEvents(Vec<Event>),
    Shutdown,
    Subscribe(FilterSet),
    Unsubscribe(FilterSet),
    SubscribeGlobalFeed(Unixtime),
    SubscribeInbox(Unixtime),
    SubscribePersonFeed(PublicKey, Unixtime),
    SubscribeReplies(IdHex),
    SubscribeRootReplies(EventReference),
    SubscribeDmChannel(DmChannel),
    SubscribeNip46,
    TempSubscribePersonFeedChunk { pubkey: PublicKey, anchor: Unixtime },
    TempSubscribeInboxFeedChunk(Unixtime),
    TempSubscribeMetadata(Vec<PublicKey>),
    UnsubscribeGlobalFeed,
    UnsubscribePersonFeed,
    UnsubscribeReplies,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum RelayConnectionReason {
    Advertising,
    Config,
    Discovery,
    FetchAugments,
    FetchDirectMessages,
    FetchContacts,
    FetchEvent,
    FetchInbox,
    FetchMetadata,
    Follow,
    Giftwraps,
    NostrConnect,
    PostEvent,
    PostContacts,
    PostLike,
    PostMetadata,
    PostMuteList,
    PostNostrConnect,
    ReadThread,
    SubscribePerson,
    SubscribeGlobal,
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
            FetchInbox => "Searching for inbox of us",
            FetchAugments => "Fetching events that augment other events (likes, zaps, deletions)",
            FetchDirectMessages => "Fetching direct messages",
            FetchEvent => "Fetching a particular event",
            FetchMetadata => "Fetching metadata for a person",
            Follow => "Following the posts of people in our Contact List",
            Giftwraps => "Fetch giftwraps addressed to you",
            NostrConnect => "Nostr connect",
            PostEvent => "Posting an event",
            Advertising => "Advertising our relay list",
            PostLike => "Posting a reaction to an event",
            FetchContacts => "Fetching our contact list",
            PostContacts => "Posting our contact list",
            PostMuteList => "Posting our mute list",
            PostMetadata => "Posting our metadata",
            PostNostrConnect => "Posting nostrconnect",
            ReadThread => "Reading ancestors to build a thread",
            SubscribePerson => "Subscribe to the events of a person",
            SubscribeGlobal => "Subscribe to the global feed on a relay",
        }
    }

    pub fn persistent(&self) -> bool {
        use RelayConnectionReason::*;
        match *self {
            Discovery => false,
            Config => false,
            FetchInbox => true,
            FetchAugments => false,
            FetchDirectMessages => true,
            FetchEvent => false,
            FetchMetadata => false,
            Follow => true,
            Giftwraps => true,
            NostrConnect => true,
            PostEvent => false,
            Advertising => false,
            PostLike => false,
            FetchContacts => false,
            PostContacts => false,
            PostMuteList => false,
            PostMetadata => false,
            PostNostrConnect => false,
            ReadThread => true,
            SubscribePerson => false,
            SubscribeGlobal => false,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct RelayJob {
    // Short reason for human viewing
    pub reason: RelayConnectionReason,

    // Payload sent when it was started
    pub(crate) payload: ToMinionPayload,
    // NOTE, there is other per-relay data stored elsewhere in
    //   overlord.minions_task_url
    //   GLOBALS.relay_picker
}

/// Lazy hash using only reason
impl Hash for RelayJob {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.reason.hash(state);
    }
}

impl RelayJob {
    // This is like equality, but ignores the random job id
    pub fn matches(&self, other: &RelayJob) -> bool {
        self.reason == other.reason && self.payload.detail == other.payload.detail
    }
}

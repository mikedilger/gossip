use crate::dm_channel::DmChannel;
use nostr_types::{
    Event, EventAddr, Id, IdHex, Metadata, MilliSatoshi, Profile, PublicKey, RelayUrl, Tag,
    UncheckedUrl,
};
use std::fmt;

/// This is a message sent to the Overlord. Tasks which take any amount of time,
/// especially involving relays, are handled by the Overlord in this way. There is
/// no return value, you'll have to check various GLOBALS state later on if you
/// depend on the result. Such an architecture works best with an immediate-mode
/// renderer.
#[derive(Debug, Clone)]
pub enum ToOverlordMessage {
    /// Calls [add_pubkey_relay](crate::Overlord::add_pubkey_relay)
    AddPubkeyRelay(PublicKey, RelayUrl),

    /// Calls [add_relay](crate::Overlord::add_relay)
    AddRelay(RelayUrl),

    /// Calls [advertise_relay_list](crate::Overlord::advertise_relay_list)
    AdvertiseRelayList,

    /// Calls [change_passphrase](crate::Overlord::change_passphrase)
    ChangePassphrase { old: String, new: String },

    /// Calls [clear_following](crate::Overlord::clear_following)
    ClearFollowing,

    /// Calls [clear_mute_list](crate::Overlord::clear_mute_list)
    ClearMuteList,

    /// Calls [delegation_reset](crate::Overlord::delegation_reset)
    DelegationReset,

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

    /// Calls [fetch_event_addr](crate::Overlord::fetch_event_addr)
    FetchEventAddr(EventAddr),

    /// Calls [follow_pubkey](crate::Overlord::follow_pubkey)
    FollowPubkey(PublicKey),

    /// Calls [follow_nip05](crate::Overlord::follow_nip05)
    FollowNip05(String),

    /// Calls [follow_nprofile](crate::Overlord::follow_nprofile)
    FollowNprofile(Profile),

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

    /// Calls [like](crate::Overlord::like)
    Like(Id, PublicKey),

    /// internal (minions use this channel too)
    MinionIsReady,

    /// internal (minions use this channel too)
    MinionJobComplete(RelayUrl, u64),

    /// internal (minions use this channel too)
    MinionJobUpdated(RelayUrl, u64, u64),

    /// Calls [pick_relays_cmd](crate::Overlord::pick_relays_cmd)
    PickRelays,

    /// Calls [post](crate::Overlord::post)
    Post {
        content: String,
        tags: Vec<Tag>,
        in_reply_to: Option<Id>,
        dm_channel: Option<DmChannel>,
    },

    /// Calls [prune_cache](crate::Overlord::prune_cache)
    PruneCache,

    /// Calls [prune_database](crate::Overlord::prune_database)
    PruneDatabase,

    /// Calls [push_follow](crate::Overlord::push_follow)
    PushFollow,

    /// Calls [push_metadata](crate::Overlord::push_metadata)
    PushMetadata(Metadata),

    /// Calls [push_mute_list](crate::Overlord::push_mute_list)
    PushMuteList,

    /// Calls [rank_relay](crate::Overlord::rank_relay)
    RankRelay(RelayUrl, u8),

    /// internal (the overlord sends messages to itself sometimes!)
    ReengageMinion(RelayUrl, Vec<RelayJob>),

    /// Calls [reresh_followed_metadata](crate::Overlord::refresh_followed_metadata)
    RefreshFollowedMetadata,

    /// Calls [repost](crate::Overlord::repost)
    Repost(Id),

    /// Calls [search](crate::Overlord::search)
    Search(String),

    /// Calls [set_active_person](crate::Overlord::set_active_person)
    SetActivePerson(PublicKey),

    /// internal
    SetThreadFeed {
        id: Id,
        referenced_by: Id,
        relays: Vec<RelayUrl>,
        author: Option<PublicKey>,
    },

    /// internal
    SetDmChannel(DmChannel),

    /// Calls [subscribe_config](crate::Overlord::subscribe_config)
    SubscribeConfig(RelayUrl),

    /// Calls [subscribe_discover](crate::Overlord::subscribe_discover)
    SubscribeDiscover(Vec<PublicKey>, Option<Vec<RelayUrl>>),

    /// Calls [shutdown](crate::Overlord::shutdown)
    Shutdown,

    /// Calls [unlock_key](crate::Overlord::unlock_key)
    UnlockKey(String),

    /// Calls [update_following](crate::Overlord::update_following)
    UpdateFollowing { merge: bool },

    /// Calls [update_metadata](crate::Overlord::update_metadata)
    UpdateMetadata(PublicKey),

    /// Calls [update_metadata_in_bulk](crate::Overlord::update_metadata_in_bulk)
    UpdateMetadataInBulk(Vec<PublicKey>),

    /// Calls [update_mute_list](crate::Overlord::update_mute_list)
    UpdateMuteList { merge: bool },

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
    pub job_id: u64,

    pub detail: ToMinionPayloadDetail,
}

#[derive(Debug, Clone)]
pub(crate) enum ToMinionPayloadDetail {
    FetchEvent(Id),
    FetchEventAddr(EventAddr),
    PostEvent(Box<Event>),
    Shutdown,
    SubscribeAugments(Vec<IdHex>),
    SubscribeOutbox,
    SubscribeDiscover(Vec<PublicKey>),
    SubscribeGeneralFeed(Vec<PublicKey>),
    SubscribeMentions,
    SubscribePersonContactList(PublicKey),
    SubscribePersonFeed(PublicKey),
    SubscribeThreadFeed(IdHex, Vec<IdHex>),
    SubscribeDmChannel(DmChannel),
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
    FetchDirectMessages,
    FetchContacts,
    FetchEvent,
    FetchMentions,
    FetchMetadata,
    Follow,
    PostEvent,
    PostContacts,
    PostLike,
    PostMetadata,
    PostMuteList,
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
            FetchDirectMessages => "Fetching direct messages",
            FetchEvent => "Fetching a particular event",
            FetchMetadata => "Fetching metadata for a person",
            PostEvent => "Posting an event",
            Advertising => "Advertising our relay list",
            PostLike => "Posting a reaction to an event",
            FetchContacts => "Fetching our contact list",
            PostContacts => "Posting our contact list",
            PostMuteList => "Posting our mute list",
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
            FetchDirectMessages => true,
            FetchEvent => false,
            FetchMetadata => false,
            PostEvent => false,
            Advertising => false,
            PostLike => false,
            FetchContacts => false,
            PostContacts => false,
            PostMuteList => false,
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
    pub(crate) payload: ToMinionPayload,
    // NOTE, there is other per-relay data stored elsewhere in
    //   overlord.minions_task_url
    //   GLOBALS.relay_picker
}

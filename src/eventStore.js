import { defineStore } from 'pinia'

export const useEventStore = defineStore('events', {
    state: () => ({
        // The event_map maps event IDs to Event structures like this
        // {
        //   id,
        //   pubkey,
        //   created_at,
        //   kind,
        //   content,
        //   replies: [],
        //   in_reply_to: null,
        //   reactions: {
        //     upvotes: 0,
        //     downvotes: 0,
        //     emojis: [  😀: 2 ],
        //   },
        //   deleted_reason: null,
        //   client: null,
        //   hashtags: [],
        //   subject: null,
        //   urls: [],
        //   last_reply_at: null
        // }
        //
        // Not included: 'raw', 'tags', 'ots'
        events: new Map(),

        // The feed is a list of event IDs to be rendered in REVERSE order
        feed: [],

        // People is a map from pubkey to Person data like this
        //    { pubkey, name, about, picture,
        //      dns_id, dns_id_valid, dns_id_last_checked, followed }
        //    These are gossip::db::DbPerson types
        people: new Map(),

        // Relay map from URL to relay data
        relays: new Map(),

        settings: {
            feed_chunk: 43200,
            overlap: 600,
            autofollow: 0
        }
    }),
})

<script setup>
    import { computed } from 'vue'
    import { useEventStore } from '../eventStore.js'
    import Name from './Name.vue'
    import Nip05 from './Nip05.vue'
    import PubKey from './PubKey.vue'
    import DateAgo from './DateAgo.vue'
    import Avatar from './Avatar.vue'
    import IconWalk from './IconWalk.vue'
    import IconQuote from './IconQuote.vue'
    import IconReply from './IconReply.vue'
    import IconRepost from './IconRepost.vue'
    import IconInfo from './IconInfo.vue'

    const props = defineProps({
        eventId: { type: String, required: true },
    })

    const store = useEventStore()

    // We won't compute this, it shouldn't change in this
    // instance of the post component
    const event = store.events.get(props.eventId);
    const ok = typeof event !== 'undefined';

    // But new post metadata might be uploaded, so we compute this
    const post_metadata = computed(() => {
        if (store.metadata.has(props.eventId)) {
            return store.metadata.get(props.eventId);
        }
        return {
            id: props.eventId,
            replies: [],
            reactions: {
                upvotes: 0,
                downvotes: 0,
                emojis: []
            }
        };
    })

    // And new person data might be uploaded, so we compute this
    // too
    const person = computed(() => {
        if (ok && store.people.has(event.pubkey)) {
            return store.people.get(event.pubkey);
        }
        return {
            pubkey: event.pubkey,
            name: "",
            about: "",
            picture: "",
            dns_id: null,
            dns_id_valid: 0,
            dns_id_last_checked: null,
            followed: 0
        };
    })
</script>

<template>
    <div v-if="ok" class="post">
        <div class="post-header">
            <div class="post-avatar">
                <Avatar :url="person.picture"></Avatar>
            </div>
            <div class="post-right-of-avatar">
                <PubKey :pubkey="event.pubkey"></PubKey>
                <span class="float-right">
                    <DateAgo :date="event.created_at"></DateAgo>
                </span>
                <br class="float-clear">
                <Name :name="person.name"></Name>
                <IconWalk v-if="person.followed"></IconWalk>
                <Nip05 :nip05="person.dns_id==null ? '' : person.dns_id" :valid="person.dns_id_valid==1"></Nip05>
                <span class="float-right faint">
                    <IconReply></IconReply>
                    <span class="space"></span>
                    <IconQuote></IconQuote>
                    <span class="space"></span>
                    <IconRepost></IconRepost>
                    <span class="space"></span>
                    <IconInfo></IconInfo>
                </span>
                <br class="float-clear">
            </div>
        </div>
        <div class="post-content">
            {{ event.content }}
        </div>
    </div>
</template>

<style scoped>
    div.post {
        padding-top: 6px;
        padding-bottom: 6px;
        border-bottom: 1px solid #505050;
    }
    div.post-header {
        display: flex;
    }
    div.post-avatar {
        flex: 0;
    }
    div.post-right-of-avatar {
        flex: 1;
    }
    div.post-content {
        color: rgba(255, 255, 255, 0.87);
        font-size: 1.15em;
        font-weight: 400;
        font-family: "Segoe UI", Roboto, Helvetica, Arial, san-serif;
        padding-top: 7px;
        padding-bottom: 3px;
        padding-left: 3em;
        white-space: pre-wrap;
        overflow-wrap: anywhere;
    }
    .faint {
        opacity: 30%;
    }
    span.space {
        padding-left: 0.5em;
        padding-right: 0.5em;
    }
    @media (prefers-color-scheme: light) {
        div.post {
            border-bottom: 1px solid #e8e8e8;
        }
        div.post-content {
            color: #5d5c61;
        }
    }
</style>

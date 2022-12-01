<script setup>
    import { computed } from 'vue'
    import { reactive } from 'vue'
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

    const pagestate = reactive({
        redraw: 1
    });

    const store = useEventStore()

    store.$subscribe((mutation, state) => {
        pagestate.redraw += 1;
    })

    let event = store.events.get(props.eventId);
    if (event == null) {
        event = {
            id: props.eventId,
            pubkey: "",
            created_at: 0,
            kind: 0,
            content: "",
            replies: [],
            in_reply_to: null,
            reactions: {
                upvotes: 0,
                downvotes: 0,
                emojis: []
            },
            deleted_reason: null,
            client: null,
            hashtags: [],
            subject: null,
            urls: []
        };
    }

    const ok = event.pubkey !== null;

    // And new person data might be uploaded, so we compute this too
    let person = store.people.get(event.pubkey);
    if (person == null) {
        person = {
            pubkey: event.pubkey,
            name: "",
            about: "",
            picture: "",
            dns_id: null,
            dns_id_valid: 0,
            dns_id_last_checked: null,
            followed: 0
        };
    }
</script>

<template>
    <div v-if="ok" class="post" :key="pagestate.redraw">
        <div>
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
                    <span class="float-right icon">
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
            <div class="post-subheader">
                <div v-if="event.deleted_reason != null">
                    <span class="deleted">DELETED:</span> {{ event.deleted_reason }}
                </div>
                <div v-if="event.hashtags.length > 0" class="hashtags float-right">
                    <span v-for="hashtag in event.hashtags" class="hashtag">#{{ hashtag }}</span>
                </div>
                <div v-if="event.subject != null" :class="event.deleted_reason!=null ? 'deleted' : ''">
                    Subject: <span class="subject">{{ event.subject }}</span>
                </div>
            </div>
            <div class="post-content" :class="event.deleted_reason!=null ? 'deleted' : ''">
                {{ event.content }}
            </div>
            <div v-if="event.replies.length>0" class="replies">
                <Post v-for="eventId in event.replies" :event-id="eventId"></Post>
            </div>
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
    span.deleted {
        color: red;
    }
    div.post-subheader {
        white-space: pre-wrap;
        overflow-wrap: anywhere;
    }
    div.deleted {
        opacity: 50%;
        text-decoration: line-through;
    }
    span.subject {
        font-weight: bold;
    }
    div.hashtags {
        font-style: italic;
    }
    span.hashtag {
        margin-left: 0.75em;
    }
    div.post-content {
        float: clear;
        color: rgba(255, 255, 255, 0.87);
        font-size: 1.2em;
        line-height: 1.4em;
        font-weight: 400;
        font-family: "Segoe UI", Roboto, Helvetica, Arial, san-serif;
        padding-top: 7px;
        padding-bottom: 3px;
        white-space: pre-wrap;
        overflow-wrap: anywhere;
    }
    .icon {
        color: #ffffff;
        opacity: 20%;
    }
    div.replies {
        margin-left: 1em;
        padding-left: 1em;
        border-left: 2px solid #b1a296;
        padding-top: 7px;
        padding-bottom: 3px;
    }
    span.space {
        padding-left: 0.5em;
        padding-right: 0.5em;
    }
    @media (prefers-color-scheme: light) {
        .icon {
            color: #000000;
        }
        div.post {
            border-bottom: 1px solid #e8e8e8;
        }
        div.post-content {
            color: #383838;
        }
    }
</style>

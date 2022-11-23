<script setup>
    import { computed } from 'vue'
    import { useEventStore } from '../eventStore.js'
    import Name from './Name.vue'
    import Nip05 from './Nip05.vue'
    import PubKey from './PubKey.vue'
    import DateAgo from './DateAgo.vue'
    import Avatar from './Avatar.vue'
    import IconWalk from './IconWalk.vue'

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
            nip05: null,
            nip05valid: false,
            followed: false
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
                <Nip05 :nip05="person.nip05==null ? '' : person.nip05" :valid="person.nip05valid"></Nip05>
            </div>
        </div>
        <div class="post-content">
            {{ event.content }}
        </div>
    </div>
</template>

<style scoped>
    div.post {
        padding-top: 4px;
        padding-bottom: 4px;
        border-bottom: 1px dotted #505050;
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
        color: white;
        font-size: 1.2em;
        padding-top: 7px;
        padding-bottom: 3px;
        padding-left: 3em;
    }
    @media (prefers-color-scheme: light) {
        div.post {
            border-bottom: 1px dotted #e8e8e8;
        }
        div.post-content {
            color: black;
        }
    }
</style>

<script setup>
    import { storeToRefs } from 'pinia'
    import { useEventStore } from '../eventStore.js'
    import Post from './Post.vue'

    const store = useEventStore()
    const { feed } = storeToRefs(store)
</script>

<template>
    <div class="main-scrollable">
        <Post v-for="eventId in feed.slice().reverse()" :event-id="eventId"></Post>
    </div>
</template>

<style scoped>
    div.main-scrollable {
        margin-top: 1em;
        padding-right: max(2em, 6vw);
        max-height: calc(100vh - 41px);
        overflow-y: scroll;
        word-break: break-word;
    }
</style>

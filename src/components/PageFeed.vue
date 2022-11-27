<script setup>
    import { storeToRefs } from 'pinia'
    import { useEventStore } from '../eventStore.js'
    import Post from './Post.vue'

    const store = useEventStore()
    const { feed } = storeToRefs(store)
</script>

<template>
    <div v-if="feed.length > 0" class="main-scrollable">
        <Post v-for="eventId in feed.slice().reverse()" :event-id="eventId"></Post>
    </div>
    <div v-else class="main-scrollable empty">
        <h3>Welcome to Gossip</h3>
        <p>Your Feed is Empty.</p>
        <p>Try <router-link to="/subscriptions">following someone</router-link></p>
        <p>Or maybe set a longer Feed Chunk Size in <router-link to="/settings">Settings</router-link></p>
    </div>
</template>

<style scoped>
    div.empty {
        padding-top: 4em;
        text-align: center;
    }
</style>

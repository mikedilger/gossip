<script setup>
    import { reactive } from 'vue'
    import { useEventStore } from '../eventStore.js'
    import Post from './Post.vue'

    const pagestate = reactive({
        redraw: 1
    });

    const store = useEventStore()

    store.$subscribe((mutation, state) => {
        pagestate.redraw += 1;
    })
</script>

<template>
    <div v-if="store.feed.length > 0" class="main-scrollable" :key="pagestate.redraw">
        <Post v-for="eventId in store.feed.slice().reverse()" :event-id="eventId"></Post>
    </div>
    <div v-else class="main-scrollable empty">
        <h3>Welcome to Gossip</h3>
        <p>Your Feed is Empty.</p>
        <p>Try <router-link to="/people">following someone</router-link></p>
        <p>Or maybe set a longer Feed Chunk Size in <router-link to="/settings">Settings</router-link></p>
    </div>
</template>

<style scoped>
    div.empty {
        padding-top: 4em;
        text-align: center;
    }
</style>

<script setup>
    import { storeToRefs } from 'pinia'
    import { useEventStore } from '../eventStore.js'

    const store = useEventStore();
    const { relays } = storeToRefs(store)
</script>

<template>
    <h2>relays</h2>
    <div class="main-scrollable">
        <div v-for="[url,relay] in relays">{{ url }}</div>
    </div>
</template>

<style scoped>
</style>


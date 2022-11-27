<script setup>
    import { reactive } from 'vue'
    import { useEventStore } from '../eventStore.js'

    let pagestate = reactive({
         redraw: 1
    });

    const store = useEventStore();

    store.$subscribe((mutation, state) => {
        // We don't know what changed, so we presume it might be
        // the relays and redraw.
        console.log(mutation); // has 'type', 'storeId', maybe 'payload' if $patch, what else?
        pagestate.redraw += 1;
    })
</script>

<template>
    <h2>relays</h2>
    <div class="main-scrollable" :key="pagestate.redraw">
        <div v-for="[url,relay] in store.relays">{{ url }}</div>
    </div>
</template>

<style scoped>
</style>


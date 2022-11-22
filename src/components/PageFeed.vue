<script setup>
    import { useEventStore } from '../eventStore.js'
    import { storeToRefs } from 'pinia'

    import Identity from './Identity.vue'
    import TextNote from './TextNote.vue'

    const store = useEventStore();
    const { textNotes } = storeToRefs(store);
</script>

<template>
    <Identity></Identity>
    <div class="main-scrollable">
        <TextNote v-for="textNote in store.textNotes" :text-note="textNote"></TextNote>
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

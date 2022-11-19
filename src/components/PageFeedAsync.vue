<script setup lang="ts">
    import Identity from './Identity.vue'
    import TextNote from './TextNote.vue'
    import { listen } from '@tauri-apps/api/event'
    import { ref } from 'vue'
</script>

<script lang="ts">
    const textNotes = ref([]);

    await listen('from_rust', (event) => {
        //console.log("from rust: ")
        //console.log(event)
        if (event.payload.kind == "event") {
            let textNote = JSON.parse(event.payload.payload);
            textNotes.value.push(textNote);
        }
    })
</script>

<template>
    <Identity></Identity>
    <div class="main-scrollable">
        <TextNote v-for="textNote in textNotes" :text-note="textNote"></TextNote>
    </div>
</template>

<style scoped>
    div.main-scrollable{
        margin-top: 1em;
        padding-right: max(2em, 6vw);
        max-height: calc(100vh - 41px);
        overflow-y: scroll;
    }
</style>

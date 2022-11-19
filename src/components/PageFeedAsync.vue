<script>
    import { listen } from '@tauri-apps/api/event'
    import { ref } from 'vue'

    const textNotes = ref([]);

    (async () => {
        await listen('from_rust', (rust_message) => {
            //console.log("from rust: ")
            //console.log(event)
            if (rust_message.payload.kind == "event") {
                let event = JSON.parse(rust_message.payload.payload);
                if (event.kind==1) {
                    textNotes.value.push(event);
                    textNotes.value.sort((a,b) => b.created_at - a.created_at);
                }
            }
        })
    })()
</script>

<script setup>
    import Identity from './Identity.vue'
    import TextNote from './TextNote.vue'
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

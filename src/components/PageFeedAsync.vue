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
                if (event.kind==0) {
                    // For every event, possibly update the name
                    textNotes.value.forEach((val, index) => {
                        if (textNotes.value[index].pubkey == event.pubkey) {
                            textNotes.value[index].name = event.name;
                        }
                    });
                }
                else if (event.kind==1) {
                    textNotes.value.push(event);
                    // resort - events may not come in sorted order every time.
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
    div.main-scrollable {
        margin-top: 1em;
        padding-right: max(2em, 6vw);
        max-height: calc(100vh - 41px);
        overflow-y: scroll;
        word-break: break-word;
    }
</style>

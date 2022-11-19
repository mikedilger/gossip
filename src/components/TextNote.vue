<script setup>
    import PubKey from './PubKey.vue'
    import DateAgo from './DateAgo.vue'
    const props = defineProps({
        textNote: { type: Object, required: true },
    })
</script>

<template>
    <div>
        <PubKey :pubkey="textNote.pubkey"></PubKey>
        <DateAgo :date="textNote.created_at"></DateAgo>
        <br>
        <div class="textnote">
            {{ textNote.content }}
        </div>
    </div>
</template>

<style scoped>
    div.textnote {
        color: black;
        font-size: 1.2em;
        border-bottom: 1px dotted black;
        padding-top: 4px;
        padding-bottom: 4px;
    }
</style>

<script setup>
    import Name from './Name.vue'
    import PubKey from './PubKey.vue'
    import DateAgo from './DateAgo.vue'
    const props = defineProps({
        textNote: { type: Object, required: true },
    })
</script>

<template>
    <div class="textnote">
        <DateAgo :date="textNote.created_at"></DateAgo>
        <Name :name="textNote.name"></Name>
        <PubKey :pubkey="textNote.pubkey"></PubKey>
        <br>
        <div class="textnote-content">
            {{ textNote.content }}
        </div>
    </div>
</template>

<style scoped>
    div.textnote {
        padding-top: 4px;
        padding-bottom: 4px;
        border-bottom: 1px dotted #D0D0D0;
    }
    div.textnote-content {
        color: black;
        font-size: 1.2em;
        padding-top: 4px;
    }
</style>

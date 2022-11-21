<script setup>
    import Name from './Name.vue'
    import PubKey from './PubKey.vue'
    import DateAgo from './DateAgo.vue'
    import Avatar from './Avatar.vue'
    const props = defineProps({
        textNote: { type: Object, required: true },
    })
</script>

<template>
    <div class="textnote">
        <div class="textnote-header">
            <div class="textnote-avatar">
                <Avatar :url="textNote.avatar_url"></Avatar>
            </div>
            <div class="textnote-right-of-avatar">
                <PubKey :pubkey="textNote.pubkey"></PubKey>
                <span class="float-right">
                    <DateAgo :date="textNote.created_at"></DateAgo>
                </span>
                <br class="float-clear">
                <Name :name="textNote.name"></Name>
            </div>
        </div>
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
    div.textnote-header {
        display: flex;
    }
    div.textnote-avatar {
        flex: 0;
    }
    div.textnote-right-of-avatar {
        flex: 1;
    }
    div.textnote-content {
        color: black;
        font-size: 1.2em;
        padding-top: 7px;
        padding-bottom: 3px;
        padding-left: 3em;
    }
</style>

<script setup>
    import Name from './Name.vue'
    import Nip05 from './Nip05.vue'
    import PubKey from './PubKey.vue'
    import DateAgo from './DateAgo.vue'
    import Avatar from './Avatar.vue'
    import IconWalk from './IconWalk.vue'
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
                <IconWalk v-if="textNote.followed"></IconWalk>
                <Nip05 :nip05="textNote.nip05==null ? '' : textNote.nip05" :valid="textNote.nip05valid"></Nip05>
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
        border-bottom: 1px dotted #505050;
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
        color: white;
        font-size: 1.2em;
        padding-top: 7px;
        padding-bottom: 3px;
        padding-left: 3em;
    }
    @media (prefers-color-scheme: light) {
        div.textnote {
            border-bottom: 1px dotted #e8e8e8;
        }
        div.textnote-content {
            color: black;
        }
    }
</style>

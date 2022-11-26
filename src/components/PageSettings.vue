<script setup>
    import { reactive, watch } from 'vue'
    import { useEventStore } from '../eventStore.js'
    import { invoke } from '@tauri-apps/api/tauri'

    const store = useEventStore()

    // Copy settings locally, but react to this local copy
    const state = reactive({
        settings: store.settings,
        saved: null,
    });

    watch(
        () => state.settings,
        () => {
            state.saved = 0;
            console.log("reset saved class");
        },
        { deep: true }
    )

    function save() {
        invoke('save_settings', { settings: state.settings })
            .then((success) => state.saved = 1)
            .catch((error) => state.saved = 2)
    }
</script>

<template>
    <h2>settings</h2>
    <div class="main-scrollable">
        <b>Feed Chunk Size:</b>
        <input type="number" v-model="state.settings.feed_chunk" />
        <blockquote>
            When the feed loads, it pulls in events going
            back at most this many seconds. (If you want to go back further, there will
            be a <em>load more</em> at the end of the feed, after which another chunk
            of posts of this size will be loaded). If you make this too big, Gossip
            will take a long time to load. Default is 43200 (which is 12 hours).
            NOTE: we have not implemented <em>load more</em> yet.
        </blockquote>

        <b>Overlap:</b>
        <input type="number" v-model="state.settings.overlap" />
        <br>
        <blockquote>
            Gossip remembers events, and it remembers the time when
            it last got an 'end of subscribed events' message. But instead of asking
            for every event since then, we back up a bit more because (1) not everybody's
            clocks are synchronized, and (2) events take some time to propogate through
            the relays. Default is 600 (which is 10 minutes).
        </blockquote>

        <b>Autofollow:</b>
        <input type="checkbox" :checked="state.settings.autofollow!=0" @click="state.settings.autofollow = 1 - state.settings.autofollow" />
        <br>
        <blockquote>
            When we receive events that refer to people we don't
            know about yet, do you want to automatically follow those people
            anonymously? The default is not to.
            NOTE: This has no effect yet because we aren't finding new people yet.
        </blockquote>

        <p class="center">
            <input v-if="state.saved==0" type="button" value="Click to Save Your Changes" @click="save" />
            <input v-else-if="state.saved==1" type="button" value="Settings have been saved" :disabled="true" />
            <input v-else-if="state.saved==2" type="button" value="Save failed!" :disabled="true" />
        </p>

    </div>
</template>

<style scoped>
    div.main-scrollable {
        margin-top: 1em;
        padding-right: max(2em, 6vw);
        max-height: calc(100vh - 41px);
        overflow-y: scroll;
    }
    p.center {
        text-align: center;
    }
</style>

<script setup>
    import { reactive } from 'vue'
    import { invoke } from '@tauri-apps/api/tauri'

    const state = reactive({
        tab: 'add_new',
        address: null,
        pubkey: null,
        relay: null,
        alert: null,
    });

    function follow_nip35() {
        invoke('follow_nip35', { address: state.address })
            .then((success) => statea.alert = "Client restart required")
            .catch((error) => state.alert = error)
    }

    function follow_key_and_relay() {
        invoke('follow_pubkey_relay', { pubkey: pubkey, relay: relay })
            .then((success) => state.alert = "Client restart required")
            .catch((error) => state.alert = error)
    }

    function follow_author() {
        invoke('follow_author', { })
            .then((success) => state.alert = "Client restart required")
            .catch((error) => state.alert = error)
    }

</script>

<template>
    <h2>people</h2>
    <div class="main-scrollable">
        <div v-if="state.alert!=null" class="center alert">
            {{ state.alert }}
        </div>

        <div>
            <button :class="state.tab=='add_new' ? 'selected' : ''"
             @click="state.tab='add_new'">Add New</button> |
            <button :class="state.tab=='following' ? 'selected' : ''"
             @click="state.tab='following'">Following</button> |
            <button :class="state.tab=='not_following' ? 'selected' : ''"
             @click="state.tab='not_following'">Not Following</button>
        </div>
        <hr>

        <div v-if="state.tab=='add_new'">
            <h2>Add New</h2>
            <div class="section">
                <h2>Enter Public Key and Relay</h2>

                <p>
                    If you have a person's public key and a relay that they post to,
                    enter that here:
                </p>
                <span>This should look like <b>f6429d976e0724fa67f496393e3696f96908f985f054a3ffc717156fe004cae6</b></span><br>
                Public Key: <input type="text" size="65" /><br>
                <br>
                <span>This should look like <b>wss://nostr-relay.example.com</b></span><br>
                Relay URL: <input type="text" size="40" /><br>
                <br>
                <div class="center follow-button">
                    <button @click="follow_key_and_relay()">Follow</button>
                </div>
            </div>
            <div class="section">
                <h2>NIP-35 DNS Identifier</h2>

                <p>
                    If someone uses NIP-35 to indicate where they can be followed, enter their
                    DNS Identifier here and we will look them up.
                </p>
                <span>This should look like <b>bob@example.com</b>, just like an email address</span><br>
                NIP-35 Identifier: <input type="text" v-model="state.nip35" size="40" />
                <br>
                <div class="center follow-button">
                    <button @click="follow_nip35()">Lookup and Follow</button>
                </div>
            </div>
            <div class="section">
                <h2>Don't Know Anybody?</h2>

                <p>
                    If you don't know anybody, you are welcome to follow me. It's easier
                    to find other people once you are inside the network. Feel free to stop
                    following me once you find other people you wish to follow.
                </p>
                <br>
                <div class="center follow-button">
                    <button @click="follow_author()">Follow the Author</button>
                </div>
            </div>
        </div>
        <div v-if="state.tab=='following'">
            <h2>Following</h2>
            <p>TBD</p>
        </div>
        <div v-if="state.tab=='not_following'">
            <h2>Not Following</h2>
            <p>TBD</p>
        </div>
    </div>
</template>

<style scoped>
    div.main-scrollable{
        margin-top: 1em;
        padding-right: max(2em, 6vw);
        max-height: calc(100vh - 41px);
        overflow-y: scroll;
    }
    div.section {
        padding-top: 1em;
    }
    div.follow-button {
        padding-top: 0.5em;
    }
    div.alert {
        font-size: 2em;
        border: 1px solid black;
    }
</style>

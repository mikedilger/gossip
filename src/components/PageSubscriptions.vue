<script setup>
    import { reactive } from 'vue'
    import { invoke } from '@tauri-apps/api/tauri'
    import { useEventStore } from '../eventStore.js'
    import Avatar from './Avatar.vue'
    import Name from './Name.vue'
    import Nip05 from './Nip05.vue'
    import IconWalk from './IconWalk.vue'
    import PubKey from './PubKey.vue'

    const pagestate = reactive({
        redraw: 1,
        tab: 'add_new',
        address: null,
        pubkey: null,
        relay: null,
        alert: null,
    });

    const store = useEventStore();

    // store.people.
    //    { pubkey, name, about, picture,
    //      dns_id, dns_id_valid, dns_id_last_checked, followed }

    function follow_nip35() {
        invoke('follow_nip35', { address: pagestate.address })
            .then((success) => pagestatea.alert = "Client restart required")
            .catch((error) => pagestate.alert = error)
    }

    function follow_key_and_relay() {
        invoke('follow_key_and_relay', { pubkey: pagestate.pubkey, relay: pagestate.relay })
            .then((success) => pagestate.alert = "Client restart required")
            .catch((error) => pagestate.alert = error)
    }

    function follow_author() {
        invoke('follow_author', { })
            .then((success) => pagestate.alert = "Client restart required")
            .catch((error) => pagestate.alert = error)
    }

</script>

<template>
    <h2>people</h2>
    <div class="main-scrollable">
        <div v-if="pagestate.alert!=null" class="center alert">
            {{ pagestate.alert }}
        </div>

        <div>
            <button :class="pagestate.tab=='add_new' ? 'selected' : ''"
             @click="pagestate.tab='add_new'">Add New</button> |
            <button :class="pagestate.tab=='following' ? 'selected' : ''"
             @click="pagestate.tab='following'">Following</button> |
            <button :class="pagestate.tab=='not_following' ? 'selected' : ''"
             @click="pagestate.tab='not_following'">Not Following</button>
        </div>
        <hr>

        <div v-if="pagestate.tab=='add_new'">
            <h2>Add New</h2>
            <div class="section">
                <h2>Enter Public Key and Relay</h2>

                <p>
                    If you have a person's public key and a relay that they post to,
                    enter that here:
                </p>
                <span>This should look like <b>f6429d976e0724fa67f496393e3696f96908f985f054a3ffc717156fe004cae6</b></span><br>
                Public Key: <input type="text" v-model="pagestate.pubkey" size="65" /><br>
                <br>
                <span>This should look like <b>wss://nostr-relay.example.com</b></span><br>
                Relay URL: <input type="text" v-model="pagestate.relay" size="40" /><br>
                <br>
                <div class="center follow-button">
                    <button @click="follow_key_and_relay()">Follow</button>
                </div>
            </div>
            <div class="section" v-if="false">
                <h2>NIP-35 DNS Identifier</h2>

                <p>
                    If someone uses NIP-35 to indicate where they can be followed, enter their
                    DNS Identifier here and we will look them up.
                </p>
                <span>This should look like <b>bob@example.com</b>, just like an email address</span><br>
                NIP-35 Identifier: <input type="text" v-model="pagestate.nip35" size="40" />
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
        <div v-if="pagestate.tab=='following'" :key="pagestate.redraw">
            <h2>Following</h2>
            <p v-for="[pubkey,person] in store.people" :key="person.pubkey">
                <div class="person-row">
                    <div class="avatar-column">
                        <Avatar :url="person.picture"></Avatar>
                    </div>
                    <div class="person-column">
                        <PubKey :pubkey="person.pubkey"></PubKey>
                        <br>
                        <Name :name="person.name"></Name>
                        <br>
                        {{ person.about }}
                    </div>
                </div>
            </p>
        </div>
        <div v-if="pagestate.tab=='not_following'">
            <h2>Not Following</h2>
            <p>TBD</p>
        </div>
    </div>
</template>

<style scoped>
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
    div.person-row {
        display: flex;
        border-bottom: 1px solid #505050;
    }
    div.avatar-column {
        flex: 0;
    }
    div.person-column {
        flex: 1;
    }
    @media (prefers-color-scheme: light) {
        div.person-row {
            border-bottom: 1px solid #e8e8e8;
        }
    }
</style>

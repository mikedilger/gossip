<script setup>
</script>

<template>
    <h2>Getting Started</h2>
    <div class="main-scrollable">
        <h3>Welcome to Gossip</h3>

        <p>
            In order to follow someone, you need to know their public key and also which relay(s) they post to. Go to the <a href="/subscriptions">Subscriptions</a> tab to follow people.
        </p>

        <p>
            In order to post messages yourself, you need to have an identity. Go to the <a href="/identities">Identities</a> tab to create one.
        </p>

        <hr>

        <p>
            Gossip is a nostr client.
        </p>

        <p>
            Nostr is a protocol and specification for storing and retrieving social media events onto servers called relays. Many users store their events onto multiple relays for reliability, censorship resistance, and to spread their reach. If you didn't store an event on a particular relay, don't expect anyone to find it there because relays normally don't share events with each other.
        </p>

        <p>
            Users are defined by their keypair, and are known by the public key of that pair. All events they generate are signed by their private key, and verifiable by their public key.
        </p>

        <p>
            In order to follow someone, you need to know their public key and also which relay(s) they post to.
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
</style>

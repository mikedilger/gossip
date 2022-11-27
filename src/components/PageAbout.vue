<script>
    import { invoke } from '@tauri-apps/api/tauri'
    let about_outer = {}
    invoke('about')
        .then((response) => about_outer = response)
</script>

<script setup>
    let about = about_outer
</script>

<template>
    <h2>about</h2>
    <div class="main-scrollable">
        <h2>{{  about.name }}</h2>
        <p>{{ about.description }}</p>

        <ul>
            <li>Version: <b>{{ about.version }}</b></li>
            <li>By: <b>{{ about.authors }}</b></li>
            <li>Repo: <a :href="about.repository">{{ about.repository }}</a></li>
            <li>Homepage: <a :href="about.homepage">{{ about.homepage }}</a></li>
            <li>License: <b>{{ about.license }}</b></li>
        </ul>

        <p>
            We are storing data on your system in this file: <b>{{ about.database_path }}</b>.
            This data is only used locally by this client - the nostr protocol does not use
            clients as a store of data for other people. We are storing your settings, your
            private and public key, information about relays, and a cache of events. We cache
            events in your feed so that we don't have to ask relays for them again, which
            means less network traffic and faster startup times.
        </p>

        <hr>
        <h2>nostr</h2>

        <p>
            Nostr is a protocol and specification for storing and retrieving social media events onto servers called relays. Many users store their events onto multiple relays for reliability, censorship resistance, and to spread their reach. If you didn't store an event on a particular relay, don't expect anyone to find it there because relays normally don't share events with each other.
        </p>

        <p>
            Users are defined by their keypair, and are known by the public key of that pair. All events they generate are signed by their private key, and verifiable by their public key.
        </p>

        <a href="https://github.com/nostr-protocol/nostr">Learn More about nostr</a>
    </div>
</template>

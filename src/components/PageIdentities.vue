<script setup>
    import { reactive } from 'vue'
    import { useEventStore } from '../eventStore.js'
    import { invoke } from '@tauri-apps/api/tauri'

    const pagestate = reactive({
        redraw: 1,
        alert: null,
        password: "",
        private_key: "",
    });

    const store = useEventStore()

    store.$subscribe((mutation, state) => {
        pagestate.redraw += 1;
    })

    function import_key() {
        invoke('import_key', { privatekey: pagestate.private_key })
            .then((public_key) => {
                store.public_key = public_key;
            })
            .catch((error) => {
                pagestate.alert = error
            })
    }

    function generate() {
        let password_copy = pagestate.password;
        pagestate.password = "00000000000000000000000";
        pagestate.password = "";
        // Unfortunately I don't know how to ensure the security once Tauri
        // get ahold of this password.
        invoke('generate', { password: password_copy })
            .then((success) => {
                password_copy = "00000000000000000000000";
                password_copy = "";
            })
            .catch((error) => {
                password_copy = "00000000000000000000000";
                password_copy = "";
                pagestate.alert = error
            })
    }

    function unlock() {
        let password_copy = pagestate.password;
        pagestate.password = "00000000000000000000000";
        pagestate.password = "";
        // Unfortunately I don't know how to ensure the security once Tauri
        // get ahold of this password.
        invoke('unlock', { password: password_copy })
            .then((success) => {
                password_copy = "00000000000000000000000";
                password_copy = "";
            })
            .catch((error) => {
                password_copy = "00000000000000000000000";
                password_copy = "";
                pagestate.alert = error
            })
    }
</script>

<template>
    <h2>yourself</h2>
    <div class="main-scrollable">
        <div v-if="pagestate.alert!=null" class="center alert">
            {{ pagestate.alert }}
        </div>

        <div v-if="store.public_key">
            Public Key: {{ store.public_key }}
        </div>
        <div v-else-if="store.need_password">
            Enter Password to Unlock Private Key:<br>
            Password: <input type="password" v-model="pagestate.password" />
            <button @click="unlock()">Unlock</button>
        </div>
        <div v-else>
            <h3>Generate a new Identity</h3>

            <div>
                <b>Weak Security</b> - Import your private Key
                <br>
                Private Key: <input type="text" v-model="pagestate.private_key" />
                <button @click="import_key()">Import</button>
                <ul>
                    <li>By using this, your private key is likely displayed on the screen</li>
                    <li>By using this, your private key probably remains in unallocated memory via the cut-n-paste buffer</li>
                </ul>
            </div>

            <div>
                <b>Medium Security</b> - Generate a private key
                <br>
                Password: <input type="password" v-model="pagestate.password" />
                <button @click="generate()">Generate</button>
                <ul>
                    <li>You will need to provide a PIN to unlock your private key each time you use it, and we will promptly forget your private key and PIN after each event is signed.</li>
                    <li>We will never display this private key on the screen.</li>
                    <li>We will never write this private key to disk in unencrypted form.</li>
                    <li>We will zero the memory that held your private key (and PIN) before freeing it</li>
                </ul>
            </div>

            <div>
                <b>Strong Security</b> - Use a physical hardware token
                <br>
                TBD.
                <ul>
                    <li>You will need a compatible physical hardware token.</li>
                    <li>You will need system libraries and configuration allowing us to access that token.</li>
                </ul>
            </div>
        </div>
    </div>
</template>

<style scoped>
    div.alert {
        font-size: 2em;
        border: 1px solid black;
    }
</style>

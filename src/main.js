import { createApp } from 'vue'
import { createPinia } from 'pinia'
import { createRouter, createWebHashHistory } from 'vue-router'
import { useEventStore } from './eventStore.js'
import { listen } from '@tauri-apps/api/event'
import './style.css'
import App from './App.vue'

import PageFeed from './components/PageFeed.vue'
import PagePeople from './components/PagePeople.vue'
import PageIdentities from './components/PageIdentities.vue'
import PageRelays from './components/PageRelays.vue'
import PageSettings from './components/PageSettings.vue'
import PageAbout from './components/PageAbout.vue'

const routes = [
    { path: '/', component: PageFeed },
    { path: '/people', component: PagePeople },
    { path: '/identities', component: PageIdentities },
    { path: '/relays', component: PageRelays },
    { path: '/settings', component: PageSettings },
    { path: '/about', component: PageAbout },
]


const router = createRouter({
    history: createWebHashHistory(),
    routes: routes
})

const pinia = createPinia()

const app = createApp(App);
app.use(router);
app.use(pinia);

// Process messages sent in from rust
(async () => {
    await listen('from_rust', (rust_message) => {

        let payload = JSON.parse(rust_message.payload.payload);
        const store = useEventStore();

        console.log("HANDLING RUST COMMAND " + rust_message.payload.kind)

        switch (rust_message.payload.kind) {
        case "setevents":
            store.$patch((state) => {
                payload.forEach(event => state.events.set(event.id, event))
            })
            break;
        case "replacefeed":
            store.$patch({ feed: payload });
            break;
        case "setpeople":
            store.$patch((state) => {
                payload.forEach(person => state.people.set(person.pubkey, person))
            })
            break;
        case "setsettings":
            store.$patch({ settings: payload });
            break;
        case "setrelays":
            store.$patch((state) => {
                payload.forEach(relay => state.relays.set(relay.url, relay))
            })
            break;
        case "needpassword":
            store.need_password = true;
            break;
        case "publickey":
            store.$patch((state) => {
                state.public_key = payload.public_key;
                state.key_security = payload.key_security;
                state.need_password = false;
            })
            break;
        default:
            console.log("UNRECOGNIZED COMMAND from_rust " + rust_message.payload.kind)
        }
    })
})()

app.mount('#app');

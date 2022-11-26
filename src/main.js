import { createApp } from 'vue'
import { createPinia } from 'pinia'
import { createRouter, createWebHashHistory } from 'vue-router'
import { useEventStore } from './eventStore.js'
import { listen } from '@tauri-apps/api/event'
import './style.css'
import App from './App.vue'

import PageFeed from './components/PageFeed.vue'
import PageGettingStarted from './components/PageGettingStarted.vue'
import PageSubscriptions from './components/PageSubscriptions.vue'
import PageIdentities from './components/PageIdentities.vue'
import PageSettings from './components/PageSettings.vue'
import PageAbout from './components/PageAbout.vue'

const routes = [
    { path: '/', component: PageFeed },
    { path: '/getting-started', component: PageGettingStarted },
    { path: '/subscriptions', component: PageSubscriptions },
    { path: '/identities', component: PageIdentities },
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
        case "addevents":
            payload.forEach(event => store.events.set(event.id, event))
            break;
        case "setmetadata":
            handle_setmetadata(payload);
            payload.forEach(metadata => store.metadata.set(metadata.id, metadata))
            break;
        case "replacefeed":
            store.$patch({ feed: payload });
            break;
        case "setpeople":
            console.log("SETTING " + payload.length + " PEOPLE");
            payload.forEach(person => store.people.set(person.pubkey, person))
            break;
        case "setsettings":
            console.log("Settings: ");
            console.log(payload);
            store.$patch({ settings: payload });
            break;
        default:
            console.log("UNRECOGNIZED COMMAND from_rust " + rust_message.payload.kind)
        }
    })
})()

app.mount('#app');

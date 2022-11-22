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
app.mount('#app');

// Process messages sent in from rust
(async () => {
    await listen('from_rust', (rust_message) => {

        console.log("message from rust")
        //console.log(event)

        const store = useEventStore();

        if (rust_message.payload.kind == "event") {
            let event = JSON.parse(rust_message.payload.payload);
            if (event.kind==0) {
                // For every event, possibly update the name
                store.textNotes.forEach((val, index) => {
                    if (store.textNotes[index].pubkey == event.pubkey) {
                        store.textNotes[index].name = event.name;
                    }
                });
            }
            else if (event.kind==1) {
                store.textNotes.push(event);
                // resort - events may not come in sorted order every time.
                store.textNotes.sort((a,b) => b.created_at - a.created_at);
            }
        }
    })
})()

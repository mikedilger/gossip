import { createApp } from 'vue'
import { createPinia } from 'pinia'
import { createRouter, createWebHashHistory } from 'vue-router'
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

const app = createApp(App)
      .use(router)
      .use(pinia)
      .mount('#app')

<script>
    import { invoke } from '@tauri-apps/api'
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
        </ul>
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

<script setup>
</script>

<template>
    <h2>about</h2>
    <div class="main-scrollable">
        <h2>Name</h2>
        <p>Description</p>

        <ul>
            <li>Version: <b>VERSION</b></li>
            <li>By: <b>AUTHORS</b></li>
            <li>Repo: <a :href="REPO">{{ REPO }}</a></li>
            <li>Homepage: <a :href="HOMEPAGE">{{ HOMEPAGE }}</a></li>
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

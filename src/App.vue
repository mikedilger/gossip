<script setup lang="ts">
    import Menu from './components/Menu.vue'
</script>

<template>
    <div class="page">
        <div class="sidebar">
            <Menu></Menu>
        </div>
        <div class="main-panel">
            <router-view></router-view>
        </div>
    </div>
</template>

<style scoped>
    div.page {
        display: flex;
        flex-flow: row nowrap;
        justify-content: space-between;
        align-items: stretch;
        gap: 1em;
    }

    div.sidebar {
        order: 1;
        flex: 1 1 20%;
        text-align: right;
        padding-top: 2em;
        padding-right: 1em;
        border-right: 1px solid #505050;
        font-size: 1.3rem;
        line-height: 1.8;
    }

    div.main-panel {
        order: 2;
        flex: 4 1 80%;
    }

    @media (prefers-color-scheme: light) {
        div.sidebar {
            border-right: 1px solid #E8E8E8;
        }
    }
</style>

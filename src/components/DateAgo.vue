<script setup>
    import { reactive, onMounted } from 'vue'

    const props = defineProps(['date'])
    const state = reactive({ ago: "", timeout: 500 })

    function updateAgo() {
        let now = Date.now() / 1000;
        let seconds = now - props.date;
        let minutes = seconds / 60;
        let hours = minutes / 60;
        let days = hours / 24;
        let years = days / 365;

        if (seconds < 45) {
            state.ago = Math.round(seconds) + "s";
            state.timeout = 500;
        } else if (seconds < 90) {
            state.ago = "1m";
            state.timeout = 1000;
        }
        else if (minutes < 45) {
            state.ago = Math.round(minutes) + "m";
            state.timeout = 500 * 60;
        } else if (minutes < 90) {
            state.ago = "1h";
            state.timeout = 1000 * 60;
        } else if (hours < 24) {
            state.ago = Math.round(hours) + "h";
            state.timeout = 500 * 60 * 60;
        } else if (hours < 42) {
            state.ago = "1d";
            state.timeout = 1000 * 60 * 60;
        } else if (days < 30) {
            state.ago = Math.round(days) + "d";
            state.timeout = 500 * 60 * 60 * 24;
        } else if (days < 45) {
            state.ago = "1m";
            state.timeout = 1000 * 60 * 60 * 24;
        } else if (days < 365) {
            state.ago = Math.round(days / 30) + "m";
            state.timeout = 500 * 60 * 60 * 24 * 30;
        } else if (years < 1.5) {
            state.ago = "1y";
            state.timeout = 1000 * 60 * 60 * 24 * 30;
        } else {
            state.ago = Math.round(years) + "y";
            state.timeout = null;
        }
    }

    onMounted(() => {
        updateAgo();
        setInterval(() => updateAgo(), state.timeout)
    })
</script>

<template>
    <span class="dateago">{{ state.ago }}</span>
</template>

<style scoped>
    span.dateago {
        display: inline-block;
        width: 4em;
    }
</style>

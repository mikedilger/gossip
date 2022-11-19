<script setup>
    import { computed } from 'vue'

    const props = defineProps(['date'])

    const ago = computed(() => {
        let now = Date.now() / 1000;
        let seconds = now - props.date;
        let minutes = seconds / 60;
        let hours = minutes / 60;
        let days = hours / 24;
        let years = days / 365;

        if (seconds < 45) return Math.round(seconds) + "s";
        if (seconds < 90) return "1m";
        if (minutes < 45) return Math.round(minutes) + "m";
        if (minutes < 90) return "1h";
        if (hours < 24) return Math.round(hours) + "h";
        if (hours < 42) return "1d";
        if (days < 30) return Math.round(days) + "d";
        if (days < 45) return "1m";
        if (days < 365) return Math.round(days / 30) + "m";
        if (years < 1.5) return "1y";
        return Math.round(years) + "y";
    })

</script>

<template>
    <span class="dateago">
        {{ ago }}
    </span>
</template>

<style scoped>
    span.dateago {
        color: #b1a296;
        padding-left: 2em;
    }
</style>

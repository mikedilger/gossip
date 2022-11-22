<script setup>
    const props = defineProps({
        name: { type: String, required: true },
    })
</script>

<template>
    <span class="name">{{ name }}</span>
</template>

<style scoped>
    span.name {
        font-size: 1.15em;
        color: #E0E0E0;
        font-weight: bold;
    }

    @media (prefers-color-scheme: light) {
        span.name {
            color: #404040;
        }
    }
</style>


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
        color: black;
        font-weight: bold;
    }
</style>

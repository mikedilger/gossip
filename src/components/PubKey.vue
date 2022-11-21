<script setup>
    const props = defineProps({
        pubkey: { type: String, required: true },
    })
</script>

<template>
    <span class="pubkey">{{ pubkey.substring(0,12) }}..{{ pubkey.substring(52,64) }}</span>
</template>

<style scoped>
    span.pubkey {
        color: #b1a296;
    }
</style>

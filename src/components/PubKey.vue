<script setup>
    const props = defineProps({
        pubkey: { type: String, required: true },
    })
</script>

<template>
    <span class="pubkey">{{ pubkey.substring(0,4) }}_{{ pubkey.substring(4,8) }} .. {{ pubkey.substring(56,60) }}_{{ pubkey.substring(60,64) }}</span>
</template>

<style scoped>
    span.pubkey {
        color: #b1a296;
    }
</style>

<script setup>
    const props = defineProps({
        pubkey: { type: String, required: true },
    })
</script>

<template>
    {{ pubkey.substring(0,5) }}..{{ pubkey.substring(59,64) }}
</template>

<script setup>
    const props = defineProps({
        url: { type: String, required: true },
    })
</script>

<template>
    <img :src="url" class="avatar" />
</template>

<style scoped>
    img.avatar {
        width: 36px;
        height: 36px;
        padding-right: 0.8em;
        display: inline-block;
        border-radius: 12px;
    }
</style>

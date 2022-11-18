<script setup lang="ts">
    import { useRouter, useRoute } from 'vue-router'
    const route = useRoute()

    defineProps({
        url: String,
        name: String
    })
</script>

<template>
    <router-link v-if="route.path!=url" :to="url">{{ name }}</router-link>
    <span v-else>{{ name }}</span>
    <br>
</template>

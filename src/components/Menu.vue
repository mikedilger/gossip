<script setup>
    import { useRouter, useRoute } from 'vue-router'
    import MenuItem from './MenuItem.vue'

    const route = useRoute()
</script>

<template>
    <MenuItem url="/" name="Feed"></MenuItem>
    <MenuItem url="/subscriptions" name="People"></MenuItem>
    <MenuItem url="/identities" name="Your Identity"></MenuItem>
    <MenuItem url="/settings" name="Settings"></MenuItem>
    <MenuItem url="/about" name="About"></MenuItem>
</template>

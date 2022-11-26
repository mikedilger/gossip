<script setup>
    import { useRouter, useRoute } from 'vue-router'
    import MenuItem from './MenuItem.vue'

    const route = useRoute()
</script>

<template>
    <MenuItem url="/" name="Feed"></MenuItem>
    <MenuItem url="/getting-started" name="Getting&nbsp;Started"></MenuItem>
    <MenuItem url="/subscriptions" name="Other People"></MenuItem>
    <MenuItem url="/identities" name="Yourself"></MenuItem>
    <MenuItem url="/settings" name="Settings"></MenuItem>
    <MenuItem url="/about" name="About"></MenuItem>
</template>

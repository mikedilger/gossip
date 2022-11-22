<script setup>
    import IconVerified from './IconVerified.vue'
    const props = defineProps({
        nip05: { type: String, required: true },
        valid: { type: Boolean, required: false, default: false },
    })
</script>

<template>
    <span class="nip05" :class="valid ? '' : 'invalid'"><IconVerified v-if="valid"></IconVerified>{{ nip05 }}</span>
</template>

<style scoped>
    span.invalid {
        text-decoration: line-through;
    }

    span.nip05 {
        color: #E0E0E0;
    }

    @media (prefers-color-scheme: light) {
        span.nip05 {
            color: #404040;
        }
    }
</style>


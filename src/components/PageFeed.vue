<script setup>
    import PageFeedAsync from './PageFeedAsync.vue'
    import Loading from './Loading.vue'
</script>

<template>
    <Suspense>
        <template #default>
            <PageFeedAsync>
            </PageFeedAsync>
        </template>
        <template #fallback>
            <Loading />
        </template>
    </Suspense>
</template>

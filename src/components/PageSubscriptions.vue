<template>
    <h2>people</h2>
    <div class="main-scrollable">
        <p>
            In order to follow someone, you need to know their public key and also which relay(s) they post to.
        </p>
    </div>
</template>

<style scoped>
    div.main-scrollable{
        margin-top: 1em;
        padding-right: max(2em, 6vw);
        max-height: calc(100vh - 41px);
        overflow-y: scroll;
    }
</style>

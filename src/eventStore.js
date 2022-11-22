import { defineStore } from 'pinia'

export const useEventStore = defineStore('events', {
    state: () => ({
        textNotes: []
    }),
})

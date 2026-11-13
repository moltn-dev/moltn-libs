<script setup lang="ts">
const props = defineProps<{ mode: 'loop' | 'throw' }>()
if (props.mode === 'loop') while (true) {}
if (props.mode === 'throw') throw new Error('boom volontaire')
</script>

<template><p>jamais rendu</p></template>

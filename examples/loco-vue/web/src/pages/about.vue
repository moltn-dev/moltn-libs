<script setup lang="ts">
import { Head, Link } from '@inertiajs/vue3'
import { route } from '../generated/moltn'
</script>

<template>
  <Head title="About" />
  <h1>About</h1>
  <p>A page without a loader: no server data, the route follows from the file name.</p>
  <Link :href="route('index')">Home</Link>
</template>

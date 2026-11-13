<script setup lang="ts">
import { Head, Link } from '@inertiajs/vue3'

defineProps<{ user: { id: number; name: string } }>()
</script>

<template>
  <Head :title="user.name" />
  <h1>{{ user.name }} (#{{ user.id }})</h1>
  <Link href="/">Retour</Link>
</template>

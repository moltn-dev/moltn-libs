<script setup lang="ts">
import { Head, Link } from '@inertiajs/vue3'
import { route, type Pages } from '../../generated/moltn'

defineProps<Pages['users/[id]']>()
</script>

<template>
  <Head :title="user.name" />
  <h1>{{ user.name }} (#{{ user.id }})</h1>
  <Link :href="route('index')">Back</Link>
</template>

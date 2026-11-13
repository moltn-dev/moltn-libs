<script setup lang="ts">
import { ref } from 'vue'
import { Head, Link, useForm } from '@inertiajs/vue3'
import { route, type Pages } from '../generated/moltn'

defineProps<Pages['index']>()
const count = ref(0)
const form = useForm({ name: '' })
const submit = () => form.post('/names', { onSuccess: () => form.reset() })
</script>

<template>
  <Head title="Home" />
  <h1>moltn</h1>
  <button id="counter" @click="count++">clicks: {{ count }}</button>
  <p><Link :href="route('users/[id]', { id: 42 })">User 42</Link></p>
  <p><Link :href="route('about')">About</Link></p>
  <form @submit.prevent="submit">
    <input v-model="form.name" name="name" placeholder="name" />
    <button type="submit" :disabled="form.processing">Add</button>
  </form>
  <ul>
    <li v-for="n in names" :key="n">{{ n }}</li>
  </ul>
</template>

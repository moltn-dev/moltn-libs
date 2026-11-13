<script setup lang="ts">
import { ref } from 'vue'
import { Head, Link, useForm } from '@inertiajs/vue3'

defineProps<{ names: string[] }>()
const count = ref(0)
const form = useForm({ name: '' })
const submit = () => form.post('/names', { onSuccess: () => form.reset() })
</script>

<template>
  <Head title="Accueil" />
  <h1>moltn spike</h1>
  <button id="counter" @click="count++">clics : {{ count }}</button>
  <p><Link href="/users/42">Utilisateur 42</Link></p>
  <form @submit.prevent="submit">
    <input v-model="form.name" name="name" placeholder="nom" />
    <button type="submit" :disabled="form.processing">Ajouter</button>
  </form>
  <ul>
    <li v-for="n in names" :key="n">{{ n }}</li>
  </ul>
</template>

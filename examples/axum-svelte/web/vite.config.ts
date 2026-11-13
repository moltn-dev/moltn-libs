import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import inertia from '@inertiajs/vite'
import moltn from '@moltn/vite'

export default defineConfig({
  plugins: [svelte(), inertia(), moltn()],
})

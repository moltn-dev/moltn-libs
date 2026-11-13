import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import inertia from '@inertiajs/vite'
import moltn from '@moltn/vite'

export default defineConfig({
  plugins: [react(), inertia(), moltn()],
})

import { createInertiaApp } from '@inertiajs/svelte'

// Même entrée pour le client et le SSR : @inertiajs/vite injecte le resolver (./pages)
// et, en build SSR, enveloppe l'appel dans createServer(renderPage).
createInertiaApp()

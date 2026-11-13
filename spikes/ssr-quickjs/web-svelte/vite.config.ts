import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import inertia from '@inertiajs/vite'
import { fileURLToPath } from 'node:url'

export default defineConfig(({ isSsrBuild }) => ({
  // Doit correspondre à asset_base de ViteRootView (préchargement des chunks).
  base: '/build/',
  plugins: [svelte(), inertia({ ssr: 'src/app.ts' })],
  // Le SSR doit tenir en un fichier sans import, exécutable par QuickJS.
  resolve: isSsrBuild
    ? { alias: { '@inertiajs/svelte/server': fileURLToPath(new URL('./src/moltn-server.ts', import.meta.url)) } }
    : {},
  // « worker » plutôt que « browser » : Svelte y sert son runtime serveur.
  ssr: { noExternal: true, target: 'webworker', resolve: { conditions: ['worker', 'production'], externalConditions: ['worker', 'production'] } },
  define: isSsrBuild ? { 'process.env.NODE_ENV': '"production"' } : {},
  build: isSsrBuild
    ? { outDir: 'dist/ssr', sourcemap: false, rolldownOptions: { output: { entryFileNames: 'ssr.js', codeSplitting: false } } }
    : { outDir: 'dist/client', manifest: true, rolldownOptions: { input: 'src/app.ts' } },
}))

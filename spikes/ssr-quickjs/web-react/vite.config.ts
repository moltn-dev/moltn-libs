import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import inertia from '@inertiajs/vite'
import { fileURLToPath } from 'node:url'

export default defineConfig(({ isSsrBuild }) => ({
  // Doit correspondre à asset_base de ViteRootView (préchargement des chunks).
  base: '/build/',
  plugins: [react(), inertia({ ssr: 'src/app.ts' })],
  // Le SSR doit tenir en un fichier sans import, exécutable par QuickJS.
  resolve: isSsrBuild
    ? {
        alias: {
          '@inertiajs/react/server': fileURLToPath(new URL('./src/moltn-server.ts', import.meta.url)),
          // renderToString seul (build « legacy », synchrone) : évite le rendu en flux, qui exige
          // MessageChannel, TextEncoder et ReadableStream dès le chargement.
          'react-dom/server': fileURLToPath(new URL('./node_modules/react-dom/cjs/react-dom-server-legacy.browser.production.js', import.meta.url)),
        },
      }
    : {},
  // « worker » plutôt que « browser » : runtime serveur des frameworks qui le proposent (Svelte).
  ssr: { noExternal: true, target: 'webworker', resolve: { conditions: ['worker', 'production'], externalConditions: ['worker', 'production'] } },
  define: isSsrBuild ? { 'process.env.NODE_ENV': '"production"' } : {},
  build: isSsrBuild
    ? { outDir: 'dist/ssr', sourcemap: false, rolldownOptions: { output: { entryFileNames: 'ssr.js', codeSplitting: false } } }
    : { outDir: 'dist/client', manifest: true, rolldownOptions: { input: 'src/app.ts' } },
}))

import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import inertia from '@inertiajs/vite'
import { fileURLToPath } from 'node:url'

export default defineConfig(({ isSsrBuild }) => ({
  // Doit correspondre à asset_base de ViteRootView (préchargement des chunks).
  base: '/build/',
  plugins: [vue(), inertia({ ssr: 'src/app.ts' })],
  // Le SSR doit tenir en un fichier sans import, exécutable par QuickJS.
  resolve: isSsrBuild
    ? { alias: { '@inertiajs/vue3/server': fileURLToPath(new URL('./src/moltn-server.ts', import.meta.url)) } }
    : {},
  // « worker » plutôt que « browser » : runtime serveur des frameworks qui le proposent (Svelte).
  ssr: { noExternal: true, target: 'webworker', resolve: { conditions: ['worker', 'production'], externalConditions: ['worker', 'production'] } },
  define: isSsrBuild ? { 'process.env.NODE_ENV': '"production"' } : { __VUE_PROD_HYDRATION_MISMATCH_DETAILS__: 'true' },
  build: isSsrBuild
    ? { outDir: 'dist/ssr', sourcemap: false, rolldownOptions: { output: { entryFileNames: 'ssr.js', codeSplitting: false } } }
    : { outDir: 'dist/client', manifest: true, rolldownOptions: { input: 'src/app.ts' } },
}))

# `@moltn/vite` — plugin Vite

## Objectif

Rendre le front d'une app Moltn aussi simple à configurer qu'une app Laravel + Inertia : on ajoute un plugin, le reste est réglé.

Il **complète** `@inertiajs/vite` (plugin officiel v3) au lieu de le remplacer. Les deux sont déclarés ensemble.

```ts
// vite.config.ts
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import inertia from '@inertiajs/vite'
import moltn from '@moltn/vite'

export default defineConfig({
  plugins: [vue(), inertia(), moltn()],
})
```

## Ce que fait déjà `@inertiajs/vite` (vérifié au spike, v3.11)

- **Entrée unique.** Une seule entrée `src/app.ts` contenant `createInertiaApp()` sert au client et au SSR. En build SSR, le plugin réécrit l'appel en :
  ```ts
  import createServer from '@inertiajs/vue3/server'   // react / svelte selon le framework
  const renderPage = async (page) => (await createInertiaApp(...))(page, renderToString)
  if (import.meta.env.PROD) createServer(renderPage)
  export default renderPage
  ```
  Il détecte l'entrée SSR parmi `src/ssr.ts`, `src/app.ts`… ou via `inertia({ ssr: 'src/app.ts' })`.
- **Résolution des pages.** `createInertiaApp()` sans `resolve` → le plugin injecte un resolver sur `./pages` (ou `pages: { directory }`). Même convention que `moltn-pages` : rien à ajouter.
- **SSR en dev.** Le serveur Vite expose `POST /__inertia_ssr` (body : page object ; réponse : `{ head, body }`, ou `null` pendant l'échauffement du graphe de modules).

## Responsabilités

### 1. Bundle SSR compatible moteur embarqué
Le bundle SSR doit être **un seul fichier ESM sans aucun import** et sans dépendance Node. Réglages appliqués par le plugin (M2, Vite 8 / rolldown) :
- `ssr: { noExternal: true, target: 'webworker', resolve: { conditions: ['worker', 'production'], externalConditions: ['worker', 'production'] } }`. La condition **`worker`** (et non `browser`) est indispensable pour Svelte, qui y sert son runtime serveur ; sans effet sur Vue et React ;
- `build.rolldownOptions.output: { entryFileNames: 'ssr.js', codeSplitting: false }` (pas de découpage) ;
- `build.outDir: 'dist/ssr'`, `build.sourcemap: false` ;
- `define: { 'process.env.NODE_ENV': '"production"' }` ;
- côté client : `build.manifest: true`, `outDir: 'dist/client'` (manifeste lu par `veer`), entrée `src/app.ts` (option `entry`), et **`base` = `asset_base` de `ViteRootView`** (option `assetBase`, défaut `/build/`), sinon le préchargement des chunks vise `/assets/…` (404). `base` n'est posé qu'au **build** : le serveur de dev reste à la racine ;
- chaque réglage cède devant une valeur explicite de l'utilisateur (`outDir`, `base`, `input`…) ;
- React (**décidé en M2**) : `react-dom/server` charge aussi le rendu en flux, qui crée un `MessageChannel` et un `TextEncoder` dès son chargement. Ils sont fournis par `moltn-ssr` (prélude) : l'entrée publique de React fonctionne telle quelle, sans dépendre d'un chemin interne de react-dom. Coût : +240 Ko de bundle, aucun sur le temps de rendu (~1,1 ms).
- Le fichier garde un `export default renderPage` : `moltn-ssr` le charge comme **module** ES.
- **Modules Node** (vérifiés à la résolution, avant tout autre plugin) :
  - import **statique** (`import fs from 'node:fs'`) → **échec du build**, avec le module, le fichier du projet (`src/pages/users/[id].vue`) ou le paquet fautif, et la marche à suivre ;
  - import **dynamique** (`import('node:async_hooks')`) → simple information : c'est une dépendance optionnelle (Svelte s'en sert ainsi et s'en passe). Il est remplacé par un module qui fait échouer l'import, comme dans un navigateur.
- **Vérification en fin de build** : un seul fichier, et aucun import restant (les imports dynamiques inlinés, qui pointent vers le fichier lui-même, sont ignorés).

### 2. `createServer` par alias
En build SSR, le plugin **aliase `@inertiajs/<framework>/server`** vers un module `@moltn/vite/server` qui :
- importe les polyfills nécessaires au rendu (voir `moltn-ssr.md`, « Environnement JS ») ;
- exporte par défaut `createServer(render)`, qui pose `globalThis.__moltnRender = render` au lieu d'ouvrir un serveur HTTP.

Conséquence : l'utilisateur **n'écrit pas de `ssr.ts`** et ne change aucun import. Son `app.ts` reste celui de la doc Inertia :

```ts
// web/src/app.ts
import { createInertiaApp } from '@inertiajs/vue3'
createInertiaApp()
```

Le shim (`packages/vite/src/server.ts`) tient en dix lignes et sert tel quel aux trois frameworks : l'alias couvre `@inertiajs/vue3/server`, `@inertiajs/react/server` et `@inertiajs/svelte/server`. Les erreurs de composant Vue sont traitées par `moltn-ssr` (tout `console.error` pendant un rendu le fait échouer), pas par le shim.

### 3. Régénération des types en dev : **retirée du plugin (décision M5)**
Lancer `moltn gen` depuis Vite compilerait l'application pendant que `moltn dev` la fait tourner : sous Windows, cargo ne peut pas réécrire un exécutable en cours d'exécution, le build échouerait. Et les types ne changent qu'avec le code Rust, qui demande de toute façon de recompiler et relancer l'application. Donc :
- l'application écrit `moltn.ts` elle-même au démarrage (debug, seulement s'il change) ;
- `moltn dev` relance l'application quand les sources Rust, `Cargo.toml`, `build.rs` ou la **liste** des pages changent (remplace cargo-watch).
Le plugin reste sans aucune dépendance au binaire `moltn`.

## Ce que le plugin ne fait pas
- Pas de serveur SSR de dev maison : en dev, soit pas de SSR, soit `/__inertia_ssr` de `@inertiajs/vite`.
- Pas de resolver de pages : celui de `@inertiajs/vite` suit déjà la convention.
- Pas de génération de types côté TS : c'est Rust qui génère, le plugin ne fait que déclencher.

## Tests
- `packages/vite/test/` (`pnpm --filter @moltn/vite test`, sur de vrais builds Vite) : import statique de module Node dans le projet ou dans un paquet → échec qui le nomme ; import dynamique optionnel → build OK et import rejeté à l'exécution ; alias de `createServer` → un seul fichier, sans import, qui définit `__moltnRender`.
- Build des trois exemples (Vue, React, Svelte) puis tests de conformité Rust qui chargent le bundle produit.
- Création d'un fichier dans `pages/` en dev : `moltn gen` appelé une seule fois (anti-rebond).

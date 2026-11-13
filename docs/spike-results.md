# Résultats du spike

Code : `spikes/ssr-quickjs/` (jetable). Un seul binaire Rust ; le front est choisi par `SPIKE_WEB=web-vue|web-react|web-svelte`.
Machine : Windows 11, 32 cœurs, Rust 1.99.1, Node 24.14.1, pnpm 11.8.0.
Versions : `veer` 0.3.0, `rquickjs` 0.14.0, Inertia 3.11.0 (`@inertiajs/vite`, `vue3`, `react`, `svelte`), Vue 3.5.45, React 19.3.1, Svelte 5.57.3, Vite 8.3.8 (rolldown).

## Verdict : **Go** pour les trois frameworks, en QuickJS

| | Vue | React | Svelte |
|---|---|---|---|
| Rendu par rquickjs dans axum + veer | OK | OK | OK |
| Hydratation sans avertissement | OK | OK | OK |
| Compteur interactif, lien Inertia, POST + redirection, SSR direct de `/users/42` | OK | OK | OK |
| Rendu, liste de 100 éléments (médiane / p99, 500 rendus) | 1,24 ms / 2,2 ms | 1,25 ms / 2,0 ms | **0,39 ms / 0,86 ms** |
| Bundle SSR (gzip) | 627 Ko (143) | 1 251 Ko (253) | 494 Ko (111) |
| Chargement du bundle par worker | ~25 ms | ~45 ms | ~20 ms |
| Boucle infinie : coupée à 500 ms, repli client, worker recréé | OK | OK | OK |
| Exception dans un composant → repli client | **non** (page partielle) | OK | OK |

Cible indicative : ~20 ms. Toutes les mesures sont 15 à 50 fois en dessous. Le plan B (deno_core) est inutile.
Binaire release : 3,6 Mo au total (axum + veer + QuickJS).

## Ce qu'on a appris

### Intégration avec Inertia : plus simple que prévu
- `@inertiajs/vite`, en build SSR, réécrit l'entrée : `createInertiaApp()` est enveloppé dans `createServer(renderPage)` importé de `@inertiajs/<framework>/server`, avec `export default renderPage`. **Un alias de ce module vers un shim moltn** (qui pose `globalThis.__moltnRender`) suffit. L'utilisateur n'écrit **pas de `ssr.ts`** et ne change aucun import : une seule entrée `src/app.ts` contenant `createInertiaApp()` sert au client et au SSR, pour les trois frameworks.
- Signature : `createServer(render, options)`, `render(page) → Promise<{ head, body }>` : identique au contrat `__moltnRender`.
- Le `body` renvoyé par Inertia contient déjà `<script data-page="app">` et `<div id="app">` ; `veer` l'insère tel quel.
- **Mode dev** : `@inertiajs/vite` expose `POST /__inertia_ssr` sur le serveur Vite (body = page object, réponse = `{ head, body }`, `null` pendant l'échauffement). Utilisable avec le `HttpSsrClient` de `veer` (feature `ssr`).
- Svelte n'a pas de composant `Head` : `<svelte:head>`.

### Config Vite commune (Vite 8, rolldown)
Identique pour les trois, au plugin de framework près :
- `ssr: { noExternal: true, target: 'webworker', resolve: { conditions: ['worker', 'production'], externalConditions: ['worker', 'production'] } }`.
  **La condition `worker` est indispensable pour Svelte** : avec la condition `browser` (défaut de `webworker`), `svelte` se résout vers son runtime client alors que `svelte/server` utilise le runtime serveur → erreur `lifecycle_outside_component`. Sans effet négatif sur Vue et React (temps identiques).
- `build.rolldownOptions.output: { entryFileNames: 'ssr.js', codeSplitting: false }`, `define process.env.NODE_ENV`.
- Alias `@inertiajs/<framework>/server` → shim moltn.
- **`base` côté client = `asset_base` de `ViteRootView`** (ici `/build/`). Sinon Vite précharge les chunks sous `/assets/…` et obtient des 404 (constaté avec React ; la page marche quand même, mais la console est polluée).
- Le bundle SSR reste un module ESM (`export default`) : le charger avec `Module::evaluate`, pas `eval`.

### React : rendu en flux à éviter
`react-dom/server` réexporte deux builds : le legacy (`renderToString`, synchrone, celui qu'utilise Inertia) et le rendu en flux, qui exige **dès le chargement** `MessageChannel`, `TextEncoder` et `ReadableStream`. Il n'existe pas d'entrée publique « legacy seule ».
- Spike : alias de `react-dom/server` vers `react-dom/cjs/react-dom-server-legacy.browser.production.js` (chemin interne, fragile d'une version à l'autre). Plus aucun polyfill spécifique, bundle 1 251 Ko au lieu de 1 492 Ko.
- Alternative pour M1 : fournir `MessageChannel`, `TextEncoder`, `TextDecoder`, `ReadableStream` dans l'environnement et garder l'entrée publique. Plus robuste aux mises à jour de React, plus de code côté moltn. **Retenu en M2** : seuls `MessageChannel` et `TextEncoder` sont nécessaires (chargement) ; rendu moyen ~1,1 ms avec l’entrée publique.
- Le bundle SSR React contient aussi `react-dom-client` (importé par `@inertiajs/react` pour `hydrateRoot`) : c'est l'essentiel de son poids. Sans impact sur le temps de rendu.

### Environnement JS nécessaire
- `URL` et `URLSearchParams` : **réellement utilisés** pendant le rendu (`<Link>` → `new URL(href, "http://localhost")`), pour les trois frameworks. Fournis par `core-js/actual/url*` importé dans le shim (+140 Ko).
- Classes testées seulement par `instanceof` (`FormData`, `Blob`, `File`, `FileList`, éléments DOM…) : des classes vides suffisent.
- `MessageChannel` minimal (livraison en microtâche) : gardé dans le prélude, inutile avec l'alias React legacy.
- `process.env.NODE_ENV`, `console` → logs Rust (logger `String(err)` en plus de `err.stack`, qui n'a pas le message sous QuickJS).
- Pas besoin de `setTimeout`, `TextEncoder` ni `queueMicrotask` (fourni par QuickJS) pour ces pages.
- Aucun bundle ne contient d'import ni de module Node.

### rquickjs
- `Module::evaluate(...).finish()` pour charger, `Promise::finish()` pour attendre le rendu (fait tourner la file de jobs).
- `set_interrupt_handler` fonctionne. Après interruption, rquickjs renvoie « blocking on a promise resulted in a dead lock » et non « interrupted » : détecter le dépassement par le temps écoulé, pas par le message. Ne jamais réutiliser un contexte interrompu.

### Points à traiter en M1 / M2
1. **Vue : une exception dans `setup` ne déclenche pas de repli client.** Vue l'attrape, la journalise et rend une page partielle. React et Svelte, eux, font rejeter le rendu. Piste : dans le shim, `app.config.errorHandler` côté SSR qui relance l'erreur.
2. Double `<title>` : celui du gabarit `veer` et celui de la page (`<Head>` / `<svelte:head>`). Voir s'il faut omettre `.title()` côté `veer` quand le SSR est actif.
3. React : tranché en M2 (polyfills `MessageChannel` + `TextEncoder`, entrée publique de react-dom).
4. Assets servis depuis le disque (`ServeDir`) dans le spike : l'embarquement passera par la feature `embed` de `veer` (`EmbeddedAssets`), pas encore testée.
5. pnpm 11 bloque l'install tant qu'un script de build n'est ni autorisé ni refusé (`allowBuilds` dans `pnpm-workspace.yaml`) : noté dans `moltn-toolchain.md`.

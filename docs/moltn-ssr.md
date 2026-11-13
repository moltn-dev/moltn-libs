# `moltn-ssr` — moteur SSR embarqué

## Objectif

Rendre côté serveur des pages Inertia v3 (Vue, React, Svelte) **dans le processus Rust**, sans serveur Node à côté, avec repli automatique en rendu client.

## État de l'art (vérifié en novembre 2026)

- Inertia v3 (sorti le 26 mars 2026) fait le SSR via un **serveur Node** (Node 22+, ou Bun) démarré à côté de l'application ; le plugin `@inertiajs/vite` produit le bundle SSR et, en dev, expose un point d'accès de rendu avec HMR. En cas d'échec du SSR, Inertia retombe sur un rendu client.
- Le point d'entrée SSR officiel enveloppe l'app dans `createServer(...)` qui ouvre un serveur HTTP. `createInertiaApp` y reçoit un `render` : `renderToString` (Vue), `ReactDOMServer.renderToString` (React), `render` de `svelte/server` (Svelte).
- `veer` (adaptateur Rust du protocole v3, axum) fait le SSR via un client HTTP vers ce serveur Node/Bun et expose un trait `SsrClient`. C'est par ce trait que Moltn s'intègre.

**Le trou** : aucun adaptateur n'embarque le rendu. C'est la seule raison d'être de ce crate.

## Contrat avec le bundle

Le bundle SSR, produit par `@moltn/vite`, est un **module ESM unique sans aucun import** (il garde un `export default`, donc il se charge comme module, pas avec `eval`) qui, à son évaluation, définit :

```ts
globalThis.__moltnRender = (page: Page) => Promise<{ head: string[]; body: string }>
```

C'est exactement la forme de la fonction passée à `createServer(render, options)` dans Inertia. `@moltn/vite` aliase `@inertiajs/<framework>/server` vers un `createServer` qui enregistre cette fonction au lieu d'ouvrir un serveur HTTP (voir `vite-plugin.md`). Le code de l'utilisateur reste celui de la doc Inertia, **sans aucun changement d'import**.

Le `body` renvoyé contient déjà `<script data-page="app">` et `<div id="app">…</div>` (construit par Inertia) ; `veer` l'insère tel quel.

## Architecture

```rust
// Interne au crate en 0.1 (une seule implémentation) ; public quand V8 arrivera.
// Pas de borne `Send` : un moteur naît et meurt sur son thread.
pub(crate) trait JsEngine: Sized + 'static {
    /// Évalue le bundle une fois ; échoue si __moltnRender n'est pas défini.
    fn load(bundle: &str, limits: &EngineLimits) -> Result<Self, SsrError> where Self: Sized;
    /// Appelle __moltnRender(page), attend la promesse, respecte le délai.
    fn render(&mut self, page_json: &str, timeout: Duration) -> Result<SsrOutput, SsrError>;
}

pub struct SsrOutput { pub head: Vec<String>, pub body: String }
```

| Feature | Moteur | Pour |
|---|---|---|
| `quickjs` (défaut) | `rquickjs` | Binaire léger (exemple axum + veer + Vue : 8,7 Mo), compilation simple, toutes plateformes. **Pas d'`Intl`.** |
| `v8` | crate `v8` (liaisons Rust de Deno) + `deno_core_icudata` | `Intl` complet, rendu 6 à 9 fois plus rapide ; binaire de 63 Mo pour le même exemple, cibles limitées aux bibliothèques V8 précompilées |

`EmbeddedSsrBuilder::engine(Engine::QuickJs | Engine::V8)` ; par défaut V8 dès que la feature `v8` est activée. Les deux features peuvent coexister ; sans aucune, erreur de compilation explicite.

**Décision (après M6, sur Aniplex) : le crate `v8` directement, pas `deno_core`.** La spec initiale citait `deno_core`. Il apporte une boucle d'événements tokio, un système d'« ops » et des centaines de dépendances, alors que le moteur n'a besoin que de six appels (compiler un module, appeler une fonction, vider les microtâches, lire une promesse, interrompre, limiter le tas). Le crate `v8` est celui que `deno_core` enveloppe ; les données ICU viennent du même éditeur (`deno_core_icudata`, version alignée sur `v8::icu::set_common_data_78`).

Mesures en release, un worker, 3 000 rendus (médiane / p99) :

| Bundle | QuickJS | V8 |
|---|---|---|
| Exemple Vue (630 Ko), liste de 100 éléments | 1,26 ms / 2,3 ms | 0,20 ms / 0,51 ms |
| Aniplex (3,4 Mo), page de connexion | 1,67 ms / 7,8 ms | 0,18 ms / 0,45 ms |
| Chargement du bundle Aniplex | 143 ms | 46 ms |
| Premier rendu (à froid) | 1,7 à 5,8 ms | 7 à 12 ms |

Les mêmes tests de robustesse tournent sur chaque moteur compilé (`tests/robustness.rs`, macro `engine_tests!`).

### Pool de workers
- Chaque worker = un thread OS dédié + son moteur + le bundle chargé une fois. Les contextes JS ne traversent jamais les threads.
- File bornée (`crossbeam-channel` ou `tokio::sync::mpsc`) ; réponse par `tokio::sync::oneshot`. L'appelant async attend sans bloquer tokio.
- Taille : configurable, défaut `max(1, cœurs / 2)`.
- File pleine → pas d'attente infinie : repli client immédiat et métrique `moltn_ssr_rejected_total`.
- Appelant parti avant son tour (future abandonnée : client déconnecté, délai HTTP) → le rendu en file est sauté.

### Robustesse
- **Délai par rendu** (défaut 500 ms) : handler d'interruption de QuickJS ; en V8, un thread de garde par moteur appelle `IsolateHandle::terminate_execution` à l'échéance (V8 n'a pas de point d'interruption à sonder).
  - QuickJS : la promesse du rendu est attendue par une boucle qui vérifie le délai entre deux microtâches. `Promise::finish` de rquickjs ne suffit pas : une chaîne sans fin de microtâches n'est pas arrêtée de façon fiable par le handler d'interruption (constaté : worker bloqué).
  - Après interruption, rquickjs ne renvoie pas « interrupted » mais une erreur de promesse bloquée (« dead lock »). **Détecter le dépassement par le temps écoulé**, pas par le message.
  - **Un rendu est fini quand la file de microtâches est vide** (règle commune aux deux moteurs). Ce qu'elle exécute après la résolution de la promesse compte pour ce rendu : un `console.error` tardif le fait échouer, lui, et pas la requête suivante.
  - Un contexte interrompu en plein rendu n'est **jamais réutilisé** : le worker est recréé (rechargement du bundle : ~25 ms pour 630 Ko).
- **Limite mémoire** par moteur (`set_memory_limit` QuickJS, `CreateParams::heap_limits` V8). En V8, un tas plein **abat le processus** par défaut : un rappel « proche de la limite » interrompt le rendu et accorde la marge nécessaire, le rendu échoue en `OutOfMemory` et le moteur est recréé.
- Worker en erreur fatale (délai, dépassement mémoire, panique) → remplacé par un nouveau ; le service continue.
- **Toute** erreur produit un repli client : HTML sans rendu serveur mais avec `<script data-page>`, Vue/React/Svelte montent côté client. Log `tracing::warn!` avec le nom du composant, métrique `moltn_ssr_fallback_total{reason}`.
- **Tout `console.error` émis pendant un rendu fait échouer ce rendu** (décision M1). Raison : une exception dans le `setup` d'un composant Vue ne fait pas rejeter le rendu ; Vue l'attrape, la journalise via `console.error` et rend une page partielle. Traiter le `console.error` dans le moteur couvre ce cas sans toucher au code de l'utilisateur ni au framework. React et Svelte rejettent déjà. Vérifié en M1 sur une vraie page Vue qui lève.

### Environnement JS fourni
Le moteur nu n'a pas d'API web. Liste constatée au spike (Vue, React, Svelte) :
- `console.*` redirigé vers `tracing` (en loggant `String(err)` **et** `err.stack` : la pile QuickJS ne contient pas le message) ;
- `globalThis.process = { env: { NODE_ENV: 'production' } }` pour les bibliothèques qui le lisent ;
- **`URL` et `URLSearchParams` : indispensables.** `<Link>` d'Inertia appelle `new URL(href, "http://localhost")` pendant le rendu. Fournis par le shim `@moltn/vite/server` (spike : `core-js/actual/url*`, +140 Ko au bundle ; une implémentation native côté Rust est une optimisation possible plus tard) ;
- **classes vides** pour les noms seulement testés par `instanceof` : `FormData`, `Blob`, `File`, `FileList`, `DOMException`, `Document`, `Element`, `HTMLElement`, `SVGElement`… (sans elles : `ReferenceError` dans `<Link>`) ;
- React : `MessageChannel` (livraison en microtâche) et `TextEncoder` (UTF-8, seul `encode` utilisé), que `react-dom/server` crée dès son chargement. Fournis par le prélude (décision M2, voir `vite-plugin.md`). `ReadableStream` n'est utilisé que par le rendu en flux, jamais appelé : non fourni ;
- `setTimeout`, `setInterval`, `clearTimeout`, `clearInterval` **inertes** (prélude) : l'appel réussit, le rappel n'est jamais exécuté. Beaucoup de bibliothèques posent un minuteur à l'initialisation (debounce, notifications) ; sans ces fonctions, c'était un `ReferenceError` et un repli client. Un rendu qui *attend* un minuteur échoue avec « awaits a promise that never resolves » ;
- `queueMicrotask` : fourni par QuickJS, ajouté par le prélude pour V8 (API web, absente du moteur nu) ;
- pas besoin : `TextDecoder`.
Pas de `fetch` : un rendu SSR ne fait pas de réseau, les données arrivent par les props.

**`Intl` : V8 seulement (constaté sur Aniplex, après M6).** QuickJS n'implémente pas ECMA-402 :
- `Intl.DateTimeFormat`, `Intl.NumberFormat`, `Intl.DisplayNames`… → `ReferenceError: Intl is not defined`, donc repli client de la page ;
- `Date.prototype.toLocaleDateString(locale, options)` et `Number.prototype.toLocaleString(locale, options)` existent mais **ignorent** leurs arguments : le serveur rend `10/31/2010` là où le navigateur rend `31 oct. 2010`, sans erreur ni repli.

Réponse retenue : le moteur V8 (ci-dessus), qui embarque ICU et rend comme le navigateur. Pas de polyfill ni de méthodes qui lèvent : ce seraient des demi-mesures.

### Intégration `veer`
- `EmbeddedSsr` implémente `veer::SsrClient` (feature `veer`, activée par défaut). Signature vérifiée (`veer` 0.3, `#[async_trait]`, trait toujours disponible, sans feature) :
  ```rust
  async fn render(&self, page: &serde_json::Value) -> Result<veer::SsrPayload, veer::ssr::SsrError>;
  // SsrPayload { head: Vec<String>, body: String } ; SsrError::{Transport, Decode}(String)
  ```
  En cas d'erreur, `veer` journalise et fait le repli client lui-même, sauf si `InertiaConfig::ssr_required(true)`.
- Le gabarit racine, l'injection du `<script data-page>`, l'échappement du JSON et le manifeste Vite sont gérés par `veer` (`ViteRootView`). Ne pas les dupliquer ; contribuer à `veer` si quelque chose manque.
- Constaté au spike : `ViteRootView::title(...)` + `<Head title>` côté page donnent **deux `<title>`**. À trancher en M1 (ne pas fixer de titre côté `veer` quand le SSR est actif, ou le signaler à `veer`).

### Embarquement du bundle
- Macro `embedded_bundle!()` : en release, `include_str!` du bundle SSR (chemin configurable, défaut `web/dist/ssr/ssr.js`) ; en debug, lecture depuis le disque au démarrage (et rechargement optionnel si le fichier change).
- Si le fichier est absent à la compilation en release : la compilation échoue avec l'erreur de `include_str!`, qui nomme le chemin attendu (pas de message « lancer `moltn build` » : il faudrait une macro procédurale, pas justifiée en 0.1). En debug, le message d'erreur au démarrage, lui, dit de lancer `moltn build`.

### Mode développement
Trois options, de la plus simple à la plus riche :
1. SSR désactivé en dev (rendu client) : boucle la plus rapide.
2. Bundle rechargé depuis le disque à chaque modification (`vite build --ssr --watch`).
3. **Recommandée** : délégation au point d'accès SSR de dev de `@inertiajs/vite`, `POST http://localhost:5173/__inertia_ssr` (body : page object ; réponse : `{ head, body }`, ou `null` pendant l'échauffement → traiter comme un repli client). Le client HTTP existe déjà : `veer::ssr::http::HttpSsrClient` (feature `ssr` de `veer`). HMR inclus, aucun code moltn à écrire. Format lu dans le code source du plugin (v3.11), pas documenté officiellement : à couvrir par un test.

## Contraintes pour le code des pages (à documenter pour les utilisateurs)

- Pas d'accès à `window`, `document`, `localStorage` hors `onMounted` / `useEffect` / `onMount` (Inertia v3 fournit `WhenMounted` pour le code navigateur uniquement).
- Pas de requête réseau pendant le rendu.
- Pas de dépendance à des modules Node (`fs`, `path`, `stream`, `crypto` de Node…).

## API publique (0.1)

```rust
let ssr = EmbeddedSsr::builder(embedded_bundle!())   // bundle obligatoire : pas d'erreur « bundle manquant » à l'exécution
    .workers(4)
    .render_timeout(Duration::from_millis(500))
    .memory_limit_mb(64)
    .build()?;                      // charge le bundle dans chaque worker, échoue tôt si invalide

let out = ssr.render(&page_json).await?;   // usage direct, hors veer
```

## Tests

- Rendu Vue, React, Svelte d'une page de démo : HTML attendu présent, hydratation sans avertissement (test navigateur headless dans les exemples).
- Boucle infinie dans un composant : délai respecté, worker recréé, repli client.
- Exception JS : repli client, log contenant le message.
- Bundle sans `__moltnRender` : `build()` échoue avec un message clair.
- 1 000 rendus concurrents : aucune fuite mémoire notable, pas de blocage.
- Banc `criterion` : médiane et p99 d'une page réaliste (liste de 100 éléments), QuickJS vs V8. *Non écrit ; les mesures du tableau ci-dessus viennent d'un test temporaire.*

# Roadmap

Chaque jalon se termine par une démo utilisable. On ne passe au suivant que quand tous ses critères sont verts.

## Spike — valider ou abandonner (avant tout le reste, quelques jours)

Prototype jetable, dans `spikes/` :
1. Une page `@inertiajs/vue3` v3 rendue par **rquickjs** dans un binaire axum + `veer`, via une implémentation minimale de `SsrClient`.
2. Même chose pour React et Svelte (une page chacun).
3. Noter : polyfills réellement nécessaires, réglages Vite indispensables, temps de rendu médian.

**Go** si : les trois frameworks rendent et s'hydratent sans avertissement de mismatch, et le rendu d'une page simple reste sous ~20 ms (indicatif) en QuickJS.
**Plan B** si QuickJS est trop lent ou incompatible : refaire le point 1 avec `deno_core` (V8).
**No-go** si aucun moteur embarqué ne rend Vue de façon fiable : on s'arrête, et on aura appris pourquoi en quelques jours.

Écrire les résultats dans `docs/spike-results.md`.

**État : Go** pour Vue, React et Svelte en QuickJS (médianes 1,2 / 1,25 / 0,4 ms, hydratation propre). Détails : `docs/spike-results.md`.

## M1 — `moltn-ssr` 0.1

- Trait `JsEngine`, implémentation QuickJS, pool de workers, délai, limite mémoire, recréation des workers.
- Repli client sur toute erreur, métriques, logs.
- `embedded_bundle!()` (disque en debug, inclus en release).
- Impl `veer::SsrClient`.
- Exemple `axum-vue` complet (navigation, formulaire POST + redirection).

**Critères** : tests de robustesse de `moltn-ssr.md` verts ; l'exemple tourne en release **sur une machine sans Node**.

**État : fait.** 13 tests de robustesse + 7 exemples rustdoc + 5 tests de conformité verts ; binaire release copié seul, lancé sans Node dans le `PATH` : rendu serveur, assets embarqués, hydratation OK.

## M2 — `@moltn/vite` 0.1 + React et Svelte

- Plugin : bundle SSR à fichier unique vérifié, alias de `@inertiajs/<framework>/server` vers le shim moltn (polyfills + `__moltnRender`).
- Exemples `axum-react` et `axum-svelte`.

**Critères** : les trois exemples se construisent avec la même config à un plugin près ; un import de module Node fait échouer le build avec un message utile.

**État : fait.** `vite.config.ts` identique pour les trois exemples à part le plugin du framework (`plugins: [vue() | react() | svelte(), inertia(), moltn()]`) ; import statique de module Node → échec qui nomme le fichier ou le paquet ; 4 tests du plugin, 3 × 5 tests de conformité verts, hydratation vérifiée dans le navigateur.

## M3 — `moltn-toolchain` 0.1

- Détection, résolution (proto, mise, Volta, `PATH`), exécution (install, scripts, processus de dev supervisé).
- Node, Bun, Deno ; npm, pnpm, Yarn, bun.
- CLI `moltn` minimale : `moltn dev`, `moltn build`, `moltn doctor`.

**Critères** : `moltn dev` lance Vite et l'app en une commande, Ctrl+C ne laisse aucun processus derrière, sur Linux, macOS et Windows ; erreurs de version actionnables.

**État : fait sous Windows**, à valider en CI sur Linux et macOS (code compilé et clippy-propre pour ces cibles, jamais exécuté). `moltn dev` : Vite + app, rendu serveur et HMR via Vite ; Ctrl+C et même `taskkill /F` ne laissent aucun processus. `moltn build` et `moltn doctor` testés sur les exemples.

## M4 — `moltn-pages` 0.1

- Scan avec toutes les conventions, conflits, `build.rs`.
- Macro `#[page]`, `router()` avec vérifications groupées, `assert_valid()`.
- Génération TS des routes (`RouteParams`, `route()`).

**Critères** : l'exemple `axum-vue` passe entièrement en routage par fichiers ; une page dynamique sans loader fait échouer le test de validation avec un message clair.

**État : fait.** `axum-vue` : pages `index`, `about` (sans loader), `users/[id]`, liens via `route()` ; le POST du formulaire reste une route axum. Message obtenu : « la page posts/[slug].vue a des paramètres ([slug]) mais aucun loader : ajouter #[moltn_pages::page("posts/[slug]")] sur un handler ». `axum-react` et `axum-svelte` restent en routes manuelles.

## M5 — Typage de bout en bout + CLI complète

- Choix définitif `specta` / `ts-rs`, export des props de chaque page et des props partagées.
- `moltn gen` et `moltn gen --check`, régénération en dev depuis le plugin Vite.
- `moltn new <nom> --front vue|react|svelte`.

**Critères** : renommer un champ d'une struct de props en Rust fait échouer `tsc` côté front ; `moltn new` + `moltn dev` donnent une app qui tourne en moins de dix minutes (le critère de réussite du projet).

**État : fait.** `ts-rs` retenu (déjà utilisé par `veer`, dont on réutilise le parcours des dépendances de types). Renommage `User.name` → `full_name` : `moltn gen --check` signale le fichier périmé, `vue-tsc` échoue sur la page. `moltn new` (Vue, React, Svelte) : ~40 s sur la machine de dev (32 cœurs, crates déjà téléchargés), `moltn dev` prêt en 3 s ensuite. Régénération en dev : par `moltn dev` (relance de l'application), pas par le plugin Vite (voir `vite-plugin.md` §3). Guide utilisateur FR/EN dans `guide/`.

## M6 — Intégrations et publication

- Exemple Loco, évaluation de l'adaptateur Ferro.
- Site de documentation, guide de démarrage, publication 0.x sur crates.io et npm.

**État : fait, sauf la publication elle-même** (action du mainteneur, voir `docs/release.md`).
- Loco 1.3.1 : exemple `examples/loco-vue`, sans code d'adaptation (`before_routes` / `after_routes`), 6 tests de conformité, `moltn dev` fonctionne.
- Ferro 0.3.10 : pas d'adaptateur possible (format de page Inertia ≤ v2, rendu synchrone sans point d'accroche, hyper au lieu d'axum) ; proposition rédigée dans `upstream.md`, non envoyée.
- Site : mdBook, livres `guide/fr` et `guide/en` construits depuis le guide ; page d'accueil `guide/index.html`.
- Publication préparée : noms libres, licences, READMEs, `cargo publish --dry-run --workspace` et `npm pack --dry-run` verts. Reste : URL du dépôt, compte et organisation npm, `cargo publish` / `npm publish`.

## M7 — Configuration `moltn.toml` (première tranche faite)

Spec : `docs/config.md`. Un fichier optionnel pour ce qui est aujourd'hui figé ou dupliqué : dossiers du front et des pages, `include` / `exclude` de pages, préfixe d'URL, fichier TypeScript généré, entrée et préfixe d'assets écrits une seule fois, ports et commande cargo de `moltn dev`, fichiers surveillés, toolchain imposée.

**Critères** : voir `docs/config.md` ; sans `moltn.toml`, rien ne change.

**État : dossiers et `moltn dev` faits** (`front.dir`, `pages.dir`, `pages.typescript`, `dev.port`, `dev.vite_port`, `dev.cargo`, `dev.watch`), vérifiés sur un projet `server/` + `web/` avec `moltn.toml` à la racine : `doctor`, `gen` depuis un sous-dossier, `dev` (relance sur source Rust et migration, nouvelle page), `build` et binaire release. Aussi : `#[page(…, ssr = false)]` et `moltn_pages::sync_typescript`. Reste : voir l'en-tête de `docs/config.md`.

## Moteur V8 (fait, après l'essai sur Aniplex)

L'essai sur une vraie application a montré que QuickJS n'a pas `Intl` : dates et nombres localisés faux ou page sans rendu serveur. Feature `v8` de `moltn-ssr` (crate `v8` + données ICU), `Engine::V8`, mêmes tests de robustesse que QuickJS (42 tests verts, 21 par moteur). Mesures et décisions : `docs/moltn-ssr.md`. Reste à décider : quel moteur `moltn new` choisit par défaut.

## Plus tard (seulement sur demande réelle)

- Macro d'actions liées aux pages (POST/PUT/DELETE).
- Installation directe de Node/Bun/Deno par `moltn-toolchain`.
- Adaptateurs actix-web, Poem.

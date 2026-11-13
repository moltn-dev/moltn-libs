# CLAUDE.md — Moltn

Outillage Rust pour Inertia v3 : SSR embarqué dans le binaire, routage par fichiers typé, pilotage de la toolchain JS. Vision et périmètre : `docs/vision.md` (`README.md` est la page publique du dépôt, en anglais). Plan : `docs/roadmap.md`.

## Principes

- **KISS et livrer.** La solution la plus simple qui passe les critères d'acceptation du jalon en cours. Pas d'abstraction « pour plus tard ».
- **Un jalon à la fois.** Ne pas attaquer un jalon suivant sans qu'on te le demande.
- **Réutiliser avant d'écrire.** Le protocole Inertia vient de `veer`, le typage de `specta` (ou `ts-rs`, voir roadmap), les versions de Node viennent de proto/mise/Volta. Si une de ces dépendances bloque, le signaler avec un exemple concret avant de la contourner.
- **Ne rien supposer sur les specs externes.** Pour Inertia v3, `@inertiajs/vite`, `veer`, rquickjs, deno_core : lire la doc officielle ou le code source (inertiajs.com/docs/v3, docs.rs) avant d'implémenter. Les exemples de code de `docs/` sont indicatifs.
- **Le périmètre de `docs/vision.md` fait foi.** Une idée hors périmètre se note dans `docs/ideas.md`, elle ne se code pas.
- Si une décision de `docs/` semble mauvaise, la signaler et proposer une alternative avant de dévier.

## Organisation du dépôt

```
crates/
  moltn-ssr/          # moteur SSR embarqué, impl de veer::SsrClient
  moltn-pages/        # scan de pages/, routeur, export des types
  moltn-pages-macros/ # #[page("...")]
  moltn-toolchain/    # détection + exécution de la toolchain JS
  moltn/              # binaire `moltn` (paquet crates.io `moltn`)
packages/
  vite/                # @moltn/vite (plugin Vite + createServer)
examples/
  axum-vue/  axum-react/  axum-svelte/  loco-vue/
docs/                  # specs internes (pour le développement de moltn)
guide/                 # guide utilisateur fr/en : à tenir à jour avec toute évolution visible
```

Workspace Cargo à la racine, workspace pnpm pour `packages/` et `examples/*/web`.

## Commandes

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features    # après `pnpm -r build` (les exemples chargent leurs bundles)
cargo run -p axum-vue                    # exemple (assets et SSR construits)
cargo run -p moltn -- dev            # depuis examples/axum-vue : Vite + app, HMR
cargo run -p moltn -- gen --check    # depuis un exemple : moltn.ts à jour ?
cargo install --path crates/moltn    # installe `moltn` (après chaque changement de la CLI)
pnpm --filter moltn-guide build      # site du guide (VitePress, guide/.vitepress/dist) ; `dev` pour l'aperçu
cargo publish --dry-run --workspace      # avant toute publication (voir docs/release.md)
cargo test -p moltn-toolchain -- --ignored   # tests réels (Node, pnpm, Vite)

pnpm install
pnpm -r build                            # paquets npm + fronts des exemples
pnpm -r typecheck
pnpm -r test                             # tests du plugin @moltn/vite
```

## Langue

- **En anglais** : tout ce que voit l'utilisateur de moltn : sortie du terminal (CLI, logs, `--help`), messages d'erreur, fichiers générés (`moltn.ts`, table des pages), gabarits de `moltn new`, texte des exemples, JSDoc de l'API publique du plugin, noms des tests affichés, **rustdoc des éléments publics** (`//!` et `///` visibles sur docs.rs, exemples compris), `description` des `Cargo.toml` et `package.json`, READMEs des paquets.
- **En français** : commentaires internes du code (`//`, doc des éléments privés), specs de `docs/`, ce fichier. Le guide existe dans les deux langues (`guide/fr`, `guide/en`).

## Conventions Rust

- Édition 2024, MSRV fixée dans `rust-toolchain.toml` et dans chaque `Cargo.toml`.
- Bibliothèques : erreurs typées avec `thiserror`, jamais d'`anyhow` dans l'API publique. `anyhow` autorisé dans `moltn` (la CLI) et les exemples.
- Pas de `unwrap()` / `expect()` hors tests et invariants documentés par un commentaire.
- `tracing` pour les logs, jamais de `println!` dans les bibliothèques.
- Ne jamais bloquer le runtime tokio : le moteur JS tourne sur des threads dédiés, les processus enfants via `tokio::process`.
- Features Cargo minimales et documentées (`quickjs` par défaut, `v8` optionnelle, `axum`, `veer`…). Chaque combinaison de features documentée doit compiler en CI.
- Toute API publique a une doc rustdoc avec un exemple qui compile (`cargo test --doc`).
- Nouvelle dépendance : justifier dans le message de commit.

## Conventions TypeScript

- TypeScript strict, ESM uniquement.
- `@moltn/vite` n'a aucune dépendance d'exécution lourde ; `@inertiajs/vite` et `vite` sont des peer dependencies.
- Le code des pages d'exemple respecte les contraintes SSR (voir `docs/moltn-ssr.md`).
- Vérification de types : `vue-tsc` (Vue) et `svelte-check` (Svelte) ne supportent pas encore TypeScript 7 : ces fronts restent en `typescript@~6`. React peut utiliser TS 7.

## Tests

- **Conformité** : chaque exemple (`axum-vue`, `axum-react`, `axum-svelte`) a un test d'intégration qui démarre le serveur, demande une page, vérifie le HTML rendu et le `<script data-page>`, puis fait une visite Inertia (en-tête `X-Inertia`) et vérifie la réponse JSON.
- **Robustesse SSR** : boucle infinie, exception JS, bundle invalide (voir `docs/moltn-ssr.md`).
- **Toolchain** : tests de détection sur des dossiers fixtures (un par gestionnaire de paquets et par runtime) ; les tests qui lancent un vrai Node/Bun/Deno sont marqués `#[ignore]` et tournent en CI dans une matrice dédiée.
- Un bug corrigé = un test qui le reproduit.

## Ce qu'il ne faut pas faire

- Pas de dépendance à Node à l'exécution d'une app Moltn en production.
- Pas d'appel à `npm`/`vite` depuis un `build.rs` par défaut (lent, réseau, casse docs.rs) : voir `docs/moltn-toolchain.md`.
- Pas de réécriture du client Inertia ni du protocole.
- Pas de support Nuxt/Next, même « juste pour voir ».

# Moltn — Vue, React et Svelte rendus côté serveur par un binaire Rust

> Dossier de specs pour démarrer avec Claude Code.
> Ordre de lecture : ce README → `CLAUDE.md` → `docs/architecture.md` → `docs/roadmap.md`, puis la spec de la brique en cours.

## En une phrase

**Laravel + Inertia, mais en Rust, et livré en un seul binaire sans Node en production.**

Le backend est en Rust (axum, ou un framework construit dessus). Il choisit une page et lui passe des props typées. Le front est en Vue, React ou Svelte. La première visite reçoit du HTML déjà rendu (SEO, affichage immédiat), puis l'application continue en navigation Inertia classique. Le rendu serveur se fait **dans le binaire**, par un moteur JavaScript embarqué.

## Pourquoi

L'écosystème Rust a déjà les fondations, mais pas l'outillage qui rend l'ensemble agréable :

| Besoin | État en novembre 2026 | Moltn |
|---|---|---|
| Protocole Inertia v3 côté serveur | Couvert : `veer` (axum), `ferro-inertia`, `axum-inertia` | On réutilise `veer` |
| Typage Rust → TypeScript | Couvert : `specta`, `ts-rs`, `typeshare` | On réutilise |
| Versions de Node/Bun/pnpm… | Couvert : proto, mise, Volta | On délègue |
| **SSR sans Node en production** | **Absent** : tous les adaptateurs passent par un serveur Node ou Bun | `moltn-ssr` |
| **Routage par fichiers côté Rust + routes TS typées** | **Absent** | `moltn-pages` |
| **`cargo run` qui lance aussi Vite, build intégré** | **Partiel** | `moltn-toolchain` + `@moltn/vite` |

L'équivalent dans l'écosystème Laravel : le serveur SSR d'Inertia, `laravel-vite-plugin` et Ziggy/Wayfinder. Moltn est cette couche, en Rust, avec en plus le binaire unique que même Laravel n'a pas.

## Les briques

| Brique | Type | Rôle |
|---|---|---|
| `moltn-ssr` | crate | Moteur JS embarqué (QuickJS par défaut : léger, sans `Intl` ; V8 en option : `Intl` complet, plus rapide, binaire plus lourd) qui rend les pages ; s'intègre à `veer` |
| `moltn-pages` | crate (+ macro) | Routage par fichiers (`pages/users/[id].vue` → `/users/{id}`), loaders Rust, génération des types TS et d'un `route()` typé |
| `moltn-toolchain` | crate | Détecte et résout runtime (Node, Bun, Deno) et gestionnaire de paquets (npm, pnpm, Yarn, Bun) ; lance install, build, serveur de dev |
| `moltn` | binaire | `moltn new`, `moltn dev`, `moltn build`, `moltn gen` : fine couche au-dessus des crates |
| `@moltn/vite` | paquet npm | Plugin Vite (build SSR adapté au moteur embarqué, régénération des types en dev) + `createServer` compatible |

Chaque brique est utilisable seule. `moltn-ssr` ne dépend pas de `moltn-pages`, et `moltn-toolchain` peut servir à n'importe quel outil Rust qui pilote un projet JS.

## Périmètre

**Dedans**
- Inertia v3, adaptateurs officiels Vue, React, Svelte (client non modifié).
- SSR embarqué avec repli automatique en rendu client.
- Routage par fichiers, loaders Rust, types TS générés (props + routes).
- Détection et résolution de la toolchain JS ; délégation à proto, mise ou Volta.
- Runtimes de build : Node, Bun, Deno.
- Intégration axum + `veer` d'abord, puis Loco et Ferro.

**Dehors, définitivement**
- Nuxt, Next, Remix, SvelteKit, et tout framework qui possède son propre serveur.
- Un protocole maison à la place d'Inertia.
- Un bundler maison (Vite fait le travail).
- Un runtime JS complet capable de remplacer Node.
- Un framework web complet (ORM, auth, queues…) : Loco et Ferro existent, Moltn s'y branche.
- Les apps de bureau ou mobiles (Tauri le fait).

**Pas maintenant**
- Téléchargement direct de Node/Bun par Moltn (la délégation suffit tant que personne ne le réclame).
- Macro d'actions (POST/PUT/DELETE) liées aux pages : au départ, les mutations sont des routes axum classiques.

## Critère de réussite du projet

> `cargo new`, ajout de Moltn, une page Vue : une app SSR livrée en **un binaire**, en **moins de dix minutes**, sans rien installer à la main en dehors d'un runtime JS pour le build.

Toute fonctionnalité qui ne sert pas cette expérience attend.

## Licence

MIT OR Apache-2.0 (convention de l'écosystème Rust), pour les crates comme pour le paquet npm.

## Sommaire

| Fichier | Contenu |
|---|---|
| `CLAUDE.md` | Règles de travail pour Claude Code |
| `docs/architecture.md` | Organisation du dépôt, flux d'une requête, rôle de chaque brique |
| `docs/moltn-ssr.md` | Moteur SSR embarqué |
| `docs/moltn-pages.md` | Routage par fichiers et typage de bout en bout |
| `docs/moltn-toolchain.md` | Détection et pilotage de Node, Bun, Deno et des gestionnaires de paquets |
| `docs/vite-plugin.md` | Paquet `@moltn/vite` |
| `docs/integrations.md` | axum + `veer`, Loco, Ferro |
| `docs/roadmap.md` | Spikes et jalons avec critères d'acceptation |
| `guide/` | **Guide utilisateur** (fr, en) : démarrer, pages, types, rendu serveur, CLI, production, Loco, dépannage ; site mdBook |
| `docs/release.md` | Procédure de publication (crates.io, npm) |
| `docs/config.md` | Proposition : configuration `moltn.toml` (jalon M7, non commencé) |

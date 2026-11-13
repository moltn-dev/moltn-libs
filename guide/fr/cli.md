# Commandes

Les commandes se lancent depuis la racine du projet (le dossier qui contient `Cargo.toml`), ou depuis n'importe quel sous-dossier d'un projet qui a un `moltn.toml`. Les valeurs par défaut ci-dessous se changent dans ce fichier : voir [Configuration](configuration.md).

## `moltn new <nom>`

Crée un projet : application Rust, front, page d'accueil et page « À propos ».

| Option | Défaut | Rôle |
|---|---|---|
| `--front vue\|react\|svelte` | `vue` | framework du front |
| `--engine quickjs\|v8` | `quickjs` | moteur du rendu serveur : V8 pour `Intl` (dates et nombres selon la langue), au prix d'environ 55 Mo de binaire. Voir [Rendu serveur](server-rendering.md) |

Installe les dépendances du front (pnpm s'il est présent, sinon npm), puis compile l'application une première fois.

## `moltn dev`

Lance Vite et l'application ; affiche l'adresse à ouvrir.

| Option | Défaut | Rôle |
|---|---|---|
| `--port` | `3000` | port de l'application, l'adresse à ouvrir (transmis par la variable `PORT`) |
| `--vite-port` | `5173` | port de Vite, utilisé en interne |
| `--web` | `web` | dossier du front |

- Front : rechargement à chaud par Vite.
- Code Rust, `Cargo.toml`, `build.rs` (liste réglable : `dev.watch`), ajout ou suppression d'une page : recompilation et relance automatiques.
- Ctrl+C : arrêt de tout, sans processus orphelin.

## `moltn build`

Installe les dépendances du front, construit le front (navigateur et rendu serveur), puis `cargo build --release`. Le binaire est dans `target/release/`. Avec la variable `CI` définie, l'installation refuse un lockfile pas à jour.

| Option | Défaut | Rôle |
|---|---|---|
| `--web` | `web` | dossier du front |

## `moltn gen`

Régénère `web/src/generated/moltn.ts` (types des props et routes) en compilant et en lançant brièvement l'application.

| Option | Rôle |
|---|---|
| `--check` | ne modifie rien ; échoue si le fichier n'est pas à jour (intégration continue) |

## `moltn doctor`

Affiche la configuration effective (`moltn.toml` ou valeurs par défaut) et la toolchain JavaScript détectée : runtime (Node, Bun, Deno), gestionnaire de paquets, versions, et quel fichier a motivé chaque choix (`package.json`, lockfile, `.nvmrc`…).

| Option | Défaut | Rôle |
|---|---|---|
| `--web` | `web` | dossier du front |
| `--json` | | sortie JSON |

moltn respecte les gestionnaires de versions : avec un `.prototools` il passe par [proto](https://moonrepo.dev/proto), avec un `mise.toml` ou `.tool-versions` par [mise](https://mise.jdx.dev), et utilise les raccourcis de [Volta](https://volta.sh) s'ils sont dans le `PATH`. Si une version ne convient pas, le message donne la commande pour installer la bonne.

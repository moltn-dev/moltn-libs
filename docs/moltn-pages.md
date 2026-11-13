# `moltn-pages` — routage par fichiers et typage de bout en bout

## Objectif

Le confort d'un Nuxt (une page = un fichier, l'URL se déduit du chemin), avec la logique serveur en Rust et un front **entièrement typé** : props de chaque page et URLs générées depuis Rust.

## Pourquoi un scan Rust plutôt que vue-router

Vue Router 5 a intégré le routage par fichiers d'unplugin-vue-router et génère un `typed-router.d.ts`. On ne l'utilise pas :
- avec Inertia, **vue-router ne tourne pas** (c'est Inertia qui navigue) ;
- ça ne couvre que Vue, pas React ni Svelte ;
- un `.d.ts` est fait pour l'éditeur, son format n'est pas un contrat stable.

On reprend en revanche **les mêmes conventions de nommage**, que les développeurs connaissent déjà.

## Conventions

Dossier par défaut : `web/src/pages/` (configurable). Extensions reconnues : `.vue`, `.tsx`, `.jsx`, `.svelte`.

| Fichier | Route axum (0.8) | Composant Inertia |
|---|---|---|
| `index.vue` | `/` | `index` |
| `about.vue` | `/about` | `about` |
| `users/index.vue` | `/users` | `users/index` |
| `users/[id].vue` | `/users/{id}` | `users/[id]` |
| `users/[[tab]].vue` | `/users` et `/users/{tab}` | `users/[[tab]]` |
| `docs/[...slug].vue` | `/docs/{*slug}` | `docs/[...slug]` |
| `(admin)/stats.vue` | `/stats` | `(admin)/stats` |
| `_components/Card.vue` | *(ignoré)* | — |

Règles :
- Dossiers et fichiers préfixés par `_` ignorés (composants internes).
- Groupes `(nom)` : n'apparaissent pas dans l'URL, servent à organiser.
- Deux fichiers qui produisent la même route → **erreur de build** listant les deux chemins.
- Le nom de composant = chemin relatif sans extension ; c'est ce que reçoit la fonction `resolve` côté front.

## Loaders Rust

Une page qui a besoin de données déclare un loader :

```rust
use moltn_pages::page;

#[derive(serde::Serialize, specta::Type)]
pub struct UserShowProps { user: UserDto, can_edit: bool }

#[page("users/[id]")]
async fn user_show(
    Path(id): Path<i64>,
    State(app): State<AppState>,
    auth: Auth,
) -> Result<UserShowProps, AppError> {
    let user = app.users.find(id).await?.ok_or(AppError::NotFound)?;
    Ok(UserShowProps { can_edit: auth.can_edit(&user), user: user.into() })
}
```

- Un loader est un handler axum ordinaire (extracteurs standards) qui renvoie `Result<P, E>` où `P: Serialize` (+ `specta::Type` en M5) et `E: IntoResponse`. La macro génère un handler qui ajoute l'extracteur `Inertia` et rend `P` sous le composant de la page.
- **État** (M4) : déduit de l'extracteur `State<T>`. Un loader sans `State` se branche dans n'importe quel `Router<S>`. Pour un sous-état (`State<Db>` avec `Db: FromRef<AppState>`), écrire `#[page("…", state = AppState)]` ; sinon `router()` signale l'incompatibilité avec cette correction.
- **Noms de paramètres** : lus dans `Path(id)` / `Path((a, b))` ; non vérifiés pour une autre forme (`Path(params): Path<Struct>`, `Option<Path<…>>` des pages à paramètre facultatif).
- La macro enregistre le loader (crate `inventory` ou équivalent) avec : la clé de page, le type de props, les noms des paramètres attendus.
- **Page sans loader** : autorisée si elle n'a pas de paramètre dynamique ; elle est rendue avec des props vides.
- La redirection, le 404, le 403 passent par le type d'erreur de l'utilisateur (`IntoResponse`), comme dans n'importe quel handler axum.

## Construction du routeur

```rust
// build.rs de l'application
fn main() {
    let pages = moltn_pages::build::scan("web/src/pages").unwrap();
    moltn_pages::build::write_ts(&pages, "web/src/generated/moltn.ts").unwrap();
}

// main.rs
let app = moltn_pages::router::<AppState>(moltn_pages::pages!())?;   // Result : vérifications au démarrage
```

1. `build.rs` scanne le dossier, valide les conventions, écrit la table des pages dans `OUT_DIR` et émet `cargo:rerun-if-changed` sur le dossier. La table est incluse **dans l'application** par la macro `pages!()` : un crate ne peut pas lire le `OUT_DIR` d'un autre, d'où l'argument de `router()` (écart à la spec initiale, qui prévoyait `router()` sans argument).
2. `router()` associe chaque page à son loader et vérifie :
   - page à paramètre dynamique **sans** loader → erreur ;
   - loader dont la clé ne correspond à **aucune** page → erreur ;
   - paramètres extraits par le loader incohérents avec ceux du chemin → erreur (vérification au mieux, sur les noms).
   Les erreurs listent tous les problèmes d'un coup, pas le premier seulement.
3. Helper de test : `moltn_pages::assert_valid::<AppState>()` à mettre dans un `#[test]` pour attraper ces erreurs en CI.

Les mutations (POST/PUT/PATCH/DELETE des formulaires) restent des **routes axum classiques** au départ. Une macro d'actions liée aux pages est hors du premier périmètre.

## Génération TypeScript

Sortie par défaut : `web/src/generated/moltn.ts` (commité dans le dépôt, pour que l'éditeur et la CI front fonctionnent sans lancer Rust).

**État M5** : tout `moltn.ts` (types des props, `Pages`, `SharedProps`, `RouteParams`, `route()`) est écrit **par l'application** : `router()` le réécrit au démarrage en debug s'il a changé, ou répond à `moltn gen` (variable `MOLTN_GEN=write|check` : écrit ou vérifie, puis quitte avec 0 / 1 / 2). Emplacement par convention : `generated/moltn.ts` à côté du dossier des pages, gravé dans la table par `build::scan`. `pages!()` renvoie un `&PageTable` (pages + chemin). `build::write_ts` de M4 est supprimé (deux écrivains du même fichier). `route()` n'exige l'objet de paramètres que si la page en a : `route('about')`, `route('users/[id]', { id: 42 })` ; TypeScript refuse un paramètre manquant, mal nommé, ou une page inconnue (vérifié avec `vue-tsc`).

**Après M6 : noms de paramètres.** La vérification « le loader extrait les mêmes paramètres que le fichier » compare les noms sans tenir compte de la casse ni des `_` : `watch/[animeId].vue` et `Path(anime_id)` désignent le même paramètre (constaté sur Aniplex : sans cela, il fallait renommer le fichier ou violer `non_snake_case`).

**Après M6 : `ssr = false` et `sync_typescript`.** `#[page("watch/[id]", ssr = false)]` rend la page sans rendu serveur (`InertiaResponse::no_ssr()` de `veer`) : lecteur vidéo, éditeur lourd. `moltn_pages::sync_typescript(pages!())` est la partie de `router()` qui écrit `moltn.ts` et répond à `moltn gen`, exposée pour les applications qui construisent leur routeur tard (après une connexion à la base).

**Après M6 : routes immédiates en dev.** Le rendu de `moltn.ts` est isolé dans un module sans dépendance au runtime (`ts_render`). `build::refresh_typescript(pages, moltn.ts)` rescanne `pages/`, relit le fichier existant, **garde les types de props déjà connus** et réécrit liste des pages et routes (nouvelle page : `{}`). `moltn dev` l'appelle dès que la liste des pages change, avant de recompiler : création, déplacement ou suppression d'une page visibles dans `moltn.ts` en 0,5 à 0,8 s (contre ~4 s après recompilation sur l'exemple). Sur un fichier à jour, le résultat est identique octet pour octet (pas de rechargement Vite inutile). Feature `runtime` (par défaut) : sans elle, `moltn-pages` se réduit au module `build` ; la CLI et les `[build-dependencies]` l'utilisent ainsi (ni axum, ni veer, ni ts-rs).

```ts
// Généré par moltn — ne pas modifier.
export interface UserShowProps { user: UserDto; can_edit: boolean }
export interface UserDto { id: number; name: string }

export interface Pages {
  'index': {}
  'users/[id]': UserShowProps
}

export interface RouteParams {
  'index': {}
  'users/[id]': { id: string | number }
  'docs/[...slug]': { slug: string[] }
}

export function route<K extends keyof RouteParams>(name: K, params: RouteParams[K], query?: Record<string, string>): string
```

Utilisation côté front :

```vue
<script setup lang="ts">
import type { Pages } from '@/generated/moltn'
import { route } from '@/generated/moltn'
const props = defineProps<Pages['users/[id]']>()
</script>
<template>
  <Link :href="route('users/[id]', { id: props.user.id })">{{ props.user.name }}</Link>
</template>
```

- **Types des props** (M5) : `ts-rs`, déjà employé par `veer` (même version, 12). La macro `#[page]` lit `Props` dans `Result<Props, E>` et enregistre `ts_type::<Props>` ; le parcours des dépendances réutilise `veer::bindings::collect_decls`. Un simple `#[derive(TS)]` suffit (pas de `#[ts(export)]`, qui écrirait des fichiers pendant les tests). Les attributs serde (`rename_all`…) sont respectés. Props partagées : `moltn_pages::shared_props!(T)` ; le fichier généré inscrit `SharedProps` dans `InertiaConfig.sharedPageProps` (augmentation de `@inertiajs/core`, qui doit être une dépendance directe du front).
- **Props partagées** (utilisateur connecté, messages flash…) : typées une fois et branchées sur le mécanisme de typage global d'Inertia v3 (vérifier comment v3 déclare les props partagées).
- **Quand générer** : la table des routes est connue au build (scan), mais les types de props demandent du code Rust compilé. Donc :
  - en debug, l'application régénère le fichier au démarrage s'il a changé ;
  - `moltn gen` compile et lance un petit binaire d'export (ou l'app en mode export) pour régénérer à la demande ;
  - `moltn gen --check` échoue en CI si le fichier commité est périmé.

## Tests

État M4 : scan (toutes les conventions, conflits listés avec les deux fichiers, segments invalides), snapshot `insta` du TypeScript, routeur de bout en bout (état, sans état, facultatif, catch-all, erreur du loader, page sans loader), erreurs groupées (4 problèmes en un message), exemple `axum-vue` : `assert_valid` et liens produits par `route()` servis par axum.

Limite connue : en dev, une nouvelle page demande de relancer l'application (le `build.rs` régénère la table) ; `cargo-watch` le fait, mais il relance aussi l'application à chaque modification d'un fichier du front. À affiner avec `moltn dev` (M5).


- Scan : fixtures couvrant chaque convention, conflits, dossiers ignorés, extensions mélangées.
- Routeur : page dynamique sans loader, loader orphelin, paramètres incohérents → erreurs groupées.
- Génération : snapshot du fichier TS (`insta`) pour une app d'exemple ; le fichier généré passe `tsc --noEmit`.
- De bout en bout : `route('users/[id]', { id: 42 })` côté TS == URL acceptée par le routeur axum.

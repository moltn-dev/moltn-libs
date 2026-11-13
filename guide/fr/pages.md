# Pages et loaders

## Une page = un fichier

Chaque fichier de `web/src/pages/` est une page. Son chemin donne l'URL :

| Fichier | URL | Remarque |
|---|---|---|
| `index.vue` | `/` | |
| `about.vue` | `/about` | |
| `users/index.vue` | `/users` | |
| `users/[id].vue` | `/users/42` | paramètre `id` |
| `users/[[tab]].vue` | `/users` et `/users/profil` | paramètre facultatif (dernier segment seulement) |
| `docs/[...slug].vue` | `/docs/guide/install` | un ou plusieurs segments (dernier segment seulement) |
| `(admin)/stats.vue` | `/stats` | `(groupe)` : range les fichiers sans changer l'URL |
| `_components/Card.vue` | — | `_` : ignoré, pour les composants internes |

Extensions reconnues : `.vue`, `.tsx`, `.jsx`, `.svelte`.

Si deux fichiers donnent la même URL (par exemple `about.vue` et `about/index.vue`), la **compilation échoue** et le message cite les deux fichiers.

## Une page sans données

Une page sans paramètre peut se passer de code Rust : le fichier suffit. Elle reçoit des props vides.

## Un loader : les données d'une page

Le loader est la fonction Rust qui prépare les données d'une page. C'est un **handler axum ordinaire** (mêmes extracteurs : `Path`, `State`, `Query`, en-têtes, authentification…) annoté avec `#[page]` :

```rust
use axum::extract::{Path, State};
use axum::http::StatusCode;
use moltn_pages::page;
use serde::Serialize;
use ts_rs::TS;

#[derive(Serialize, TS)]
struct UserProps {
    user: User,
    can_edit: bool,
}

#[page("users/[id]")]
async fn user_show(
    Path(id): Path<u32>,
    State(app): State<AppState>,
) -> Result<UserProps, StatusCode> {
    let user = app.users.find(id).await.ok_or(StatusCode::NOT_FOUND)?;
    Ok(UserProps { can_edit: false, user })
}
```

Règles :

- le texte de `#[page("…")]` est le nom de la page : son chemin dans `pages/`, sans extension ;
- la fonction est `async` et renvoie `Result<Props, E>` :
  - `Props` dérive `Serialize` et `TS` (voir [Types et routes](types.md)) ;
  - `E` est n'importe quelle réponse axum (`StatusCode`, une redirection, votre type d'erreur…) : c'est elle qui est renvoyée en cas d'erreur ;
- un loader peut être placé dans n'importe quel module du crate.

Côté front, la page reçoit les props :

```vue
<script setup lang="ts">
import type { Pages } from '../../generated/moltn'
defineProps<Pages['users/[id]']>()
</script>

<template>
  <h1>{{ user.name }}</h1>
</template>
```

## L'état de l'application

L'état (connexion à la base, configuration…) est celui d'axum. moltn le déduit de l'extracteur `State<T>` du loader. Si un loader lit une **partie** de l'état (`State<Db>` avec `Db: FromRef<AppState>`), indiquez l'état complet :

```rust
#[page("users/[id]", state = AppState)]
async fn user_show(State(db): State<Db>, Path(id): Path<u32>) -> Result<UserProps, AppError> { … }
```

## Brancher le routeur

Dans `main.rs` :

```rust
let app = moltn_pages::router::<AppState>(moltn_pages::pages!())?
    .route("/users", post(create_user))     // routes axum classiques à côté
    .layer(veer::InertiaLayer::new(inertia))
    .with_state(state);
```

Et dans `build.rs` (déjà présent dans un projet créé par `moltn new`) :

```rust
fn main() {
    if let Err(e) = moltn_pages::build::scan_project() {
        panic!("{e}");
    }
}
```

## Pages sans rendu serveur

Une page qui n'a de sens que dans le navigateur (lecteur vidéo, éditeur lourd) peut sauter le rendu serveur ; elle est alors rendue par le navigateur, comme lors d'une navigation Inertia :

```rust
#[page("watch/[id]", ssr = false)]
async fn watch(Path(id): Path<u32>) -> Result<WatchProps, AppError> { … }
```

## Une page « introuvable »

moltn ne réserve aucun nom pour la page 404. Créez `pages/_404.vue` (le `_` l'exclut des routes, mais le front sait la charger) et rendez-la depuis une route attrape-tout, qui ne répond que si aucune autre route ne correspond :

```rust
async fn not_found(inertia: veer::Inertia) -> impl IntoResponse {
    inertia.render("_404", serde_json::json!({})).status(StatusCode::NOT_FOUND)
}

let pages = moltn_pages::router::<AppState>(moltn_pages::pages!())?
    .route("/{*path}", axum::routing::any(not_found));
```

## Protéger des pages

Un loader est un handler axum : un extracteur d'authentification suffit, et son refus (redirection vers la connexion, 403) devient la réponse. Attention aux pages **sans loader** : elles sont servies à tout le monde. Pour protéger tout un ensemble de pages, loaders ou non, posez un middleware sur le routeur des pages :

```rust
let pages = moltn_pages::router::<AppState>(moltn_pages::pages!())?
    .route_layer(axum::middleware::from_fn_with_state(state.clone(), require_session));
```

## Vérifications au démarrage

`router()` contrôle que pages et loaders correspondent, et liste **tous** les problèmes d'un coup (les messages de moltn sont en anglais) :

```
invalid moltn pages:
  - page posts/[slug].vue has parameters ([slug]) but no loader: add #[moltn_pages::page("posts/[slug]")] to a handler
  - loader orphan (src/main.rs:17) targets page "profil", which does not exist (expected file: pages/profil.vue, .tsx, .jsx or .svelte)
  - loader wrong_param (src/main.rs:23) extracts Path(slug) but users/[id].vue defines [id]
```

Pour attraper ces erreurs en intégration continue, ajoutez ce test :

```rust
#[test]
fn pages_valides() {
    moltn_pages::assert_valid::<AppState>(moltn_pages::pages!());
}
```

## Formulaires et actions

Les envois de formulaires (POST, PUT, DELETE…) sont des **routes axum classiques**, ajoutées à côté du routeur des pages. Avec `veer`, l'extracteur `InertiaForm` lit le formulaire et `redirect` renvoie vers une page :

```rust
async fn create_user(
    inertia: veer::Inertia,
    State(app): State<AppState>,
    veer::InertiaForm(form): veer::InertiaForm<NewUser>,
) -> veer::InertiaResponse {
    app.users.create(form).await;
    inertia.redirect("/users")
}
```

Côté front, `useForm` d'Inertia s'utilise comme d'habitude.

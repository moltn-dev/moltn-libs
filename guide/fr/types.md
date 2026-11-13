# Types et routes

Les types côté front sont **générés depuis le code Rust**. Vous ne les écrivez jamais à la main, et un changement côté Rust qui casse le front est détecté par la vérification de types.

## Le fichier généré

`web/src/generated/moltn.ts` contient :

- les types des props de chaque page, traduits depuis vos structures Rust ;
- `Pages` : les props de chaque page, par nom ;
- `SharedProps` : les props partagées par toutes les pages ;
- `RouteParams` et `route()` : des liens typés.

Avec `moltn dev`, il se met à jour tout seul :

- **créer, déplacer ou supprimer une page** : la liste des pages et `route()` sont à jour en moins d'une seconde, sans attendre la compilation ; une nouvelle page a des props `{}` le temps que l'application recompile ;
- **modifier le code Rust** : les types des props suivent dès que l'application a recompilé (quelques secondes).

Hors de `moltn dev`, `moltn gen` le régénère à la demande : il lance l'application, qui écrit le fichier dans `moltn_pages::router()` puis quitte. Si votre routeur est construit tard (après une connexion à la base, par exemple), appelez `moltn_pages::sync_typescript(moltn_pages::pages!())` tout en haut de `main` : `moltn gen` n'a alors besoin de rien d'autre. Commitez-le : l'éditeur et la vérification de types du front fonctionnent sans lancer Rust.

## Typer les props

Dérivez `TS` (crate [`ts-rs`](https://docs.rs/ts-rs)) à côté de `Serialize` :

```rust
#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
struct UserProps {
    user: User,
    is_admin: bool,      // devient isAdmin côté TypeScript
}

#[derive(Serialize, TS)]
struct User {
    id: u32,
    name: String,
}
```

Les attributs serde (`rename_all`, `rename`, `skip`…) sont respectés. Les types utilisés par les props (`User` ici) sont générés aussi.

Côté front :

```vue
<!-- Vue -->
<script setup lang="ts">
import type { Pages } from '../generated/moltn'
const props = defineProps<Pages['users/[id]']>()
</script>
```

```tsx
// React
import type { Pages } from '../generated/moltn'
export default function Show({ user }: Pages['users/[id]']) { … }
```

```svelte
<!-- Svelte -->
<script lang="ts">
  import type { Pages } from '../generated/moltn'
  let { user }: Pages['users/[id]'] = $props()
</script>
```

Renommez `name` en `full_name` côté Rust : au prochain démarrage, `moltn.ts` change, et `vue-tsc` / `tsc` / `svelte-check` signalent chaque usage de `user.name` dans le front.

## Liens typés : `route()`

```ts
import { route } from '../generated/moltn'

route('about')                                   // "/about"
route('users/[id]', { id: 42 })                  // "/users/42"
route('users/[id]', { id: 42 }, { tab: 'profil' }) // "/users/42?tab=profil"
route('docs/[...slug]', { slug: ['guide', 'install'] }) // "/docs/guide/install"
```

TypeScript refuse une page inconnue, un paramètre manquant ou mal nommé. Les valeurs sont encodées pour l'URL.

```vue
<Link :href="route('users/[id]', { id: user.id })">{{ user.name }}</Link>
```

## Props partagées

Les props communes à toutes les pages (utilisateur connecté, nom de l'application, messages flash…) se déclarent une fois :

```rust
#[derive(Serialize, TS)]
struct Shared {
    app_name: String,
}
moltn_pages::shared_props!(Shared);

let inertia = veer::InertiaConfig::new()
    .share(|_req| async { Shared { app_name: "Mon app".into() } });
```

Le fichier généré inscrit `Shared` dans le typage global d'Inertia : `usePage().props.app_name` est connu de TypeScript.

Cela demande que `@inertiajs/core` soit une dépendance directe du front (c'est le cas des projets créés par `moltn new`) :

```bash
pnpm add -D @inertiajs/core
```

## En intégration continue

```bash
moltn gen --check
```

Échoue si `moltn.ts` n'est pas à jour avec le code Rust. Ajoutez ensuite la vérification de types du front (`pnpm typecheck`) pour détecter les incompatibilités.

## Limites

- Deux structures de même nom dans des modules différents produisent deux types TypeScript de même nom : renommez l'une (`#[ts(rename = "…")]`).
- Vue et Svelte : `vue-tsc` et `svelte-check` ne supportent pas encore TypeScript 7 ; les projets restent en TypeScript 6.

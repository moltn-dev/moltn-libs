# Types and routes

Frontend types are **generated from the Rust code**. You never write them by hand, and a Rust change that breaks the frontend is caught by the type check.

## The generated file

`web/src/generated/moltn.ts` contains:

- the props types of every page, translated from your Rust structs;
- `Pages`: each page's props, by name;
- `SharedProps`: the props shared by every page;
- `RouteParams` and `route()`: typed links.

With `moltn dev`, it updates itself:

- **create, move or delete a page**: the page list and `route()` are up to date in under a second, without waiting for compilation; a new page has `{}` props until the application has recompiled;
- **edit Rust code**: props types follow as soon as the application has recompiled (a few seconds).

Outside `moltn dev`, `moltn gen` regenerates it on demand: it starts the application, which writes the file in `moltn_pages::router()` and exits. If your router is built late (after a database connection, for instance), call `moltn_pages::sync_typescript(moltn_pages::pages!())` at the very top of `main`: `moltn gen` then needs nothing else. Commit it: the editor and the frontend type check work without running Rust.

## Typing props

Derive `TS` (crate [`ts-rs`](https://docs.rs/ts-rs)) next to `Serialize`:

```rust
#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
struct UserProps {
    user: User,
    is_admin: bool,      // becomes isAdmin in TypeScript
}

#[derive(Serialize, TS)]
struct User {
    id: u32,
    name: String,
}
```

serde attributes (`rename_all`, `rename`, `skip`…) are honored. Types used by the props (`User` here) are generated too.

On the frontend:

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

Rename `name` to `full_name` in Rust: on the next start `moltn.ts` changes, and `vue-tsc` / `tsc` / `svelte-check` flag every use of `user.name` in the frontend.

## Typed links: `route()`

```ts
import { route } from '../generated/moltn'

route('about')                                     // "/about"
route('users/[id]', { id: 42 })                    // "/users/42"
route('users/[id]', { id: 42 }, { tab: 'profile' }) // "/users/42?tab=profile"
route('docs/[...slug]', { slug: ['guide', 'install'] }) // "/docs/guide/install"
```

TypeScript rejects an unknown page and a missing or misnamed parameter. Values are URL-encoded.

```vue
<Link :href="route('users/[id]', { id: user.id })">{{ user.name }}</Link>
```

## Shared props

Props common to every page (signed-in user, application name, flash messages…) are declared once:

```rust
#[derive(Serialize, TS)]
struct Shared {
    app_name: String,
}
moltn_pages::shared_props!(Shared);

let inertia = veer::InertiaConfig::new()
    .share(|_req| async { Shared { app_name: "My app".into() } });
```

The generated file registers `Shared` in Inertia's global typing: `usePage().props.app_name` is known to TypeScript.

This requires `@inertiajs/core` as a direct dependency of the frontend (projects created by `moltn new` have it):

```bash
pnpm add -D @inertiajs/core
```

## In continuous integration

```bash
moltn gen --check
```

Fails if `moltn.ts` is out of date with the Rust code. Then run the frontend type check (`pnpm typecheck`) to catch mismatches.

## Limitations

- Two structs with the same name in different modules produce two TypeScript types with the same name: rename one (`#[ts(rename = "…")]`).
- Vue and Svelte: `vue-tsc` and `svelte-check` do not support TypeScript 7 yet; those projects stay on TypeScript 6.

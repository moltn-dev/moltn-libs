# Rendu serveur

## Comment ça marche

À la première visite d'une page, l'application Rust :

1. appelle le loader de la page ;
2. fait rendre la page en HTML par un **moteur JavaScript embarqué** (QuickJS ou V8, voir plus bas), qui exécute le code de vos pages ;
3. renvoie une page HTML complète, que Vue, React ou Svelte « reprennent » ensuite dans le navigateur (hydratation).

Les navigations suivantes passent par Inertia : seules les données (JSON) transitent.

Aucun serveur Node n'est nécessaire en production. Rendre une page prend de l'ordre d'une milliseconde.

En développement, `moltn dev` fait rendre les pages par Vite, pour profiter du rechargement à chaud.

## Si le rendu échoue

Une exception dans un composant, un rendu trop long, un dépassement de mémoire : la page est envoyée **sans rendu serveur**, et le navigateur l'affiche normalement. Rien ne casse pour le visiteur ; l'erreur est journalisée côté serveur avec le nom de la page.

Un `console.error` émis pendant le rendu compte aussi comme un échec : c'est ainsi que Vue signale une erreur dans un composant.

## Règles pour le code des pages

Le code des pages s'exécute aussi sur le serveur, dans un moteur JavaScript sans navigateur ni Node :

- **pas de `window`, `document`, `localStorage`** pendant le rendu : utilisez-les dans `onMounted` (Vue), `useEffect` (React) ou `onMount` (Svelte) ;
- **pas de requête réseau** pendant le rendu : les données arrivent par les props, depuis le loader Rust ;
- **pas de module Node** (`fs`, `path`, `stream`, `crypto`…), ni de bibliothèque qui en dépend. Le build échoue avec un message qui nomme le fichier ou le paquet fautif.
- **pas d'état propre à un visiteur dans une variable de module** (un `ref` déclaré hors composant, une instance i18n globale) : le serveur garde le même moteur d'une requête à l'autre, la valeur d'un visiteur serait servie au suivant. Réalignez cet état au début de chaque rendu à partir des props, par exemple dans `withApp` de `createInertiaApp` (langue, utilisateur connecté).
- **avec QuickJS, pas d'`Intl` ni de formatage selon la langue** : `Intl.DateTimeFormat` n'existe pas (la page part sans rendu serveur) et `toLocaleDateString(langue, options)` ignore la langue (le serveur écrit `10/31/2010`, le navigateur corrige en `31 oct. 2010`). Le moteur V8 n'a pas cette limite : voir « Choisir le moteur ».

`URL`, `URLSearchParams` et `TextEncoder` sont disponibles.

`setTimeout` et `setInterval` existent mais **ne se déclenchent jamais** côté serveur : le rendu s'arrête dès que le HTML est produit. Une bibliothèque qui pose un minuteur fonctionne donc ; un rendu qui en *attend* un (`await` sur un `setTimeout`) échoue et la page part sans rendu serveur.

## Choisir le moteur

Deux moteurs JavaScript sont disponibles, au choix dans `Cargo.toml` :

| | QuickJS (par défaut) | V8 |
|---|---|---|
| Taille ajoutée au binaire | environ 1 Mo | environ 55 Mo |
| `Intl`, dates et nombres selon la langue | **non** | oui, comme le navigateur |
| Rendu d'une page | de l'ordre de 1 ms | de l'ordre de 0,2 ms |
| Plateformes | toutes | celles pour lesquelles V8 est précompilé |

Prenez **V8** dès que vos pages affichent des dates ou des nombres selon la langue (`Intl.DateTimeFormat`, `toLocaleDateString`, `toLocaleString`) : avec QuickJS ces appels échouent ou ignorent la langue.

```toml
moltn-ssr = { version = "0.1", default-features = false, features = ["veer", "v8"] }
```

Un nouveau projet l'obtient d'emblée avec `moltn new mon-app --engine v8`. Rien d'autre à changer : avec la feature `v8`, `EmbeddedSsr::builder(…)` utilise V8. Pour choisir explicitement : `.engine(moltn_ssr::Engine::V8)`.

## Réglages

Le moteur se configure à la création, dans `main.rs` :

```rust
use std::time::Duration;

let ssr = moltn_ssr::EmbeddedSsr::builder(moltn_ssr::embedded_bundle!())
    .workers(4)                                   // moteurs en parallèle (défaut : la moitié des cœurs)
    .render_timeout(Duration::from_millis(500))   // durée maximale d'un rendu (défaut : 500 ms)
    .memory_limit_mb(64)                          // mémoire maximale par moteur (défaut : 64 Mo)
    .build()?;
```

Quand tous les moteurs sont occupés et que la file d'attente est pleine, la page part sans rendu serveur plutôt que d'attendre.

## Mesures

moltn publie trois métriques via la crate [`metrics`](https://docs.rs/metrics), à brancher sur l'exportateur de votre choix (Prometheus…) :

- `moltn_ssr_render_seconds` : durée des rendus réussis ;
- `moltn_ssr_fallback_total{reason}` : rendus en échec, par cause ;
- `moltn_ssr_rejected_total` : rendus refusés faute de place.

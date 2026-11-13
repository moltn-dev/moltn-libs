# Architecture

## Vue d'ensemble

```
                         ┌──────────────────── binaire de l'application (Rust) ─────────────────────┐
 Navigateur ── GET /users/42 ──▶ axum ──▶ moltn-pages : route générée → loader Rust (#[page])     │
                         │                     │ props typées                                       │
                         │                     ▼                                                    │
                         │               veer (protocole Inertia v3)                                │
                         │      visite Inertia ? ──oui──▶ JSON { component, props, url, version }   │
                         │            │ non (première visite)                                       │
                         │            ▼                                                             │
                         │      moltn-ssr (impl SsrClient) ──▶ pool de moteurs JS embarqués        │
                         │            │ { head, body }            (bundle SSR embarqué)             │
                         │            ▼                                                             │
                         │      HTML complet + <script data-page> + assets Vite embarqués           │
                         └──────────────────────────────────────────────────────────────────────────┘

 Développement / build (jamais en production) :
   moltn ────── moltn-toolchain ──▶ node | bun | deno  +  npm | pnpm | yarn | bun ──▶ vite (@moltn/vite)
                └── moltn-pages (scan) ──▶ types TS générés (props, routes) ──▶ front
```

## Rôle de chaque brique

### `moltn-ssr` (production)
Rend un page object Inertia en HTML, dans le processus Rust. Ne connaît ni axum, ni les pages, ni le framework front : il exécute un bundle qui expose une fonction de rendu. Détails : `moltn-ssr.md`.

### `moltn-pages` (production + génération)
- **Au build** (`build.rs` de l'application) : scanne `pages/`, génère la table des routes (chemins, paramètres, nom de composant).
- **À l'exécution** : construit un `axum::Router` à partir de cette table et des loaders déclarés par `#[page("...")]`, vérifie au démarrage que tout correspond.
- **En dev / CI** : exporte les types TS (props de chaque page, paramètres de route, fonction `route()`).
Détails : `moltn-pages.md`.

### `moltn-toolchain` (dev + build)
Trouve quel runtime et quel gestionnaire de paquets le projet front attend, vérifie qu'ils sont disponibles (directement ou via proto/mise/Volta), et lance `install`, `build`, `dev`. Aucun rôle en production. Détails : `moltn-toolchain.md`.

### `moltn` : la CLI (dev + build)
Commandes pour humains et CI, sans logique propre : tout est dans les crates.
- `moltn new <nom> --front vue|react|svelte` : squelette d'application.
- `moltn dev` : lance Vite (via toolchain) et l'application Rust (`cargo watch -x run` si cargo-watch est installé, sinon `cargo run`), régénère les types (M5). `bacon` écarté : interface plein écran incompatible avec la sortie de Vite.
- `moltn doctor` : toolchain détectée, d'où vient chaque décision, ce qui manque.
- `moltn build` : build front (client + SSR) puis `cargo build --release` avec embarquement.
- `moltn gen [--check]` : régénère les types TS ; `--check` échoue si les fichiers commités sont périmés (CI).

### `@moltn/vite` (dev + build)
Plugin Vite qui complète `@inertiajs/vite` : produit un bundle SSR exécutable par le moteur embarqué (un seul fichier, aucun import Node), remplace par alias le `createServer` d'Inertia (aucun `ssr.ts` à écrire), aligne `base` sur `asset_base` de `veer`. Détails : `vite-plugin.md`.

## Ce que fait l'application de l'utilisateur

```rust
// main.rs (indicatif)
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let ssr = moltn_ssr::EmbeddedSsr::builder()
        .bundle(moltn_ssr::embedded_bundle!())   // dist/ssr/ssr.js embarqué en release
        .build()?;

    let inertia = veer::InertiaConfig::new()
        .ssr(ssr)
        /* manifeste Vite, version, gabarit racine : via veer */;

    let app = moltn_pages::router::<AppState>()?  // routes générées depuis pages/
        .route("/api/health", axum::routing::get(|| async { "ok" }))
        .layer(veer::InertiaLayer::new(inertia))
        .with_state(AppState::new().await?);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;
    Ok(())
}
```

## Modes d'exécution

| | Dev (`moltn dev`) | Release |
|---|---|---|
| Assets front | Serveur Vite (HMR) | Embarqués dans le binaire |
| SSR | Point d'accès `POST /__inertia_ssr` de `@inertiajs/vite` via `HttpSsrClient` de `veer` (HMR inclus). `moltn dev` pose `MOLTN_VITE_URL` ; l'application choisit dev ou embarqué selon cette variable (voir les exemples) | Moteur embarqué |
| Types TS | Écrits par l'application à chaque démarrage ; `moltn dev` la relance quand le Rust ou la liste des pages change | Vérifiés en CI (`moltn gen --check`) |
| Node/Bun/Deno | Requis | **Absent** |

## Décisions structurantes

1. **Protocole délégué à `veer`.** Moltn ne réimplémente pas Inertia. Si `veer` devient un problème (maintenance, licence, API), on écrira un cœur de protocole indépendant du routeur sur le modèle de `deno-inertia`, mais pas avant.
2. **Moteur derrière un trait.** QuickJS par défaut (léger, compilation simple), V8 en option. Le reste du code ne sait pas lequel tourne.
3. **Contrat de rendu indépendant du framework.** Le bundle expose `globalThis.__moltnRender(page) → Promise<{ head: string[], body: string }>`. Vue, React et Svelte ne diffèrent que par leur point d'entrée SSR.
4. **Types générés depuis Rust, jamais écrits à la main.** Les structs Rust sont la source de vérité.
5. **Pas de Node en production, pas de `npm` dans `build.rs` par défaut.**

//! Gabarits de `moltn new`. Marqueurs remplacés : `{{name}}`, `{{moltn_pages}}`, `{{moltn_pages_build}}`,
//! `{{moltn_ssr}}`, `{{moltn_vite}}`, `{{react_refresh}}`.

/// Framework du front.
#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Front {
    Vue,
    React,
    Svelte,
}

/// Moteur JavaScript du rendu serveur (feature Cargo de `moltn-ssr`).
#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Engine {
    /// Small binary, builds everywhere; no `Intl` (locale-aware dates and numbers).
    Quickjs,
    /// Full `Intl` and faster rendering; adds about 55 MB to the binary.
    V8,
}

/// (chemin, contenu) des fichiers d'un nouveau projet.
pub fn files(front: Front) -> Vec<(&'static str, &'static str)> {
    let mut files = vec![
        ("Cargo.toml", CARGO_TOML),
        ("build.rs", BUILD_RS),
        ("moltn.toml", MOLTN_TOML),
        ("src/main.rs", MAIN_RS),
        (".gitignore", GITIGNORE),
    ];
    let web: [(&str, &str); 6] = match front {
        Front::Vue => [
            ("web/package.json", VUE_PACKAGE),
            ("web/vite.config.ts", VUE_VITE),
            ("web/tsconfig.json", TSCONFIG),
            (
                "web/src/app.ts",
                "import { createInertiaApp } from '@inertiajs/vue3'\n\ncreateInertiaApp()\n",
            ),
            ("web/src/pages/index.vue", VUE_INDEX),
            ("web/src/pages/about.vue", VUE_ABOUT),
        ],
        Front::React => [
            ("web/package.json", REACT_PACKAGE),
            ("web/vite.config.ts", REACT_VITE),
            ("web/tsconfig.json", REACT_TSCONFIG),
            (
                "web/src/app.ts",
                "import { createInertiaApp } from '@inertiajs/react'\n\ncreateInertiaApp()\n",
            ),
            ("web/src/pages/index.tsx", REACT_INDEX),
            ("web/src/pages/about.tsx", REACT_ABOUT),
        ],
        Front::Svelte => [
            ("web/package.json", SVELTE_PACKAGE),
            ("web/vite.config.ts", SVELTE_VITE),
            ("web/tsconfig.json", TSCONFIG),
            (
                "web/src/app.ts",
                "import { createInertiaApp } from '@inertiajs/svelte'\n\ncreateInertiaApp()\n",
            ),
            ("web/src/pages/index.svelte", SVELTE_INDEX),
            ("web/src/pages/about.svelte", SVELTE_ABOUT),
        ],
    };
    files.extend(web);
    files
}

const CARGO_TOML: &str = r#"[package]
name = "{{name}}"
version = "0.1.0"
edition = "2024"
publish = false

# Standalone project, even when created inside another Cargo workspace.
[workspace]

[dependencies]
anyhow = "1"
axum = "0.8"
moltn-pages = {{moltn_pages}}
moltn-ssr = {{moltn_ssr}}
rust-embed = "8"
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread", "net"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
ts-rs = "12"
veer = { version = "0.3", features = ["axum", "embed", "ssr"] }

[build-dependencies]
moltn-pages = {{moltn_pages_build}}
"#;

const MOLTN_TOML: &str = r#"# moltn configuration. Every key is optional: these are the defaults.

# [front]
# dir = "web"                             # frontend folder, relative to this file

# [pages]
# dir = "src/pages"                       # relative to front.dir
# typescript = "src/generated/moltn.ts"   # relative to front.dir

# [dev]
# port = 3000
# vite_port = 5173
# cargo = ["run"]                         # cargo arguments that run the application
# watch = ["src", "Cargo.toml", "build.rs"]   # changes that restart it
"#;

const BUILD_RS: &str = r#"//! File-based routing: page table from the pages folder (`web/src/pages/`, see moltn.toml).

fn main() {
    if let Err(e) = moltn_pages::build::scan_project() {
        panic!("{e}");
    }
}
"#;

const MAIN_RS: &str = r#"//! `moltn dev` to develop, `moltn build` for the production binary.

use std::convert::Infallible;

use anyhow::Context as _;
use moltn_pages::page;
use moltn_ssr::EmbeddedSsr;
use serde::Serialize;
use ts_rs::TS;
use veer::ssr::http::HttpSsrClient;
use veer::{EmbeddedAssets, InertiaConfig, InertiaLayer, ViteManifest, ViteRootView};

/// State shared by the loaders (database pool, configuration…).
#[derive(Clone, Default)]
struct AppState;

/// Props of the `web/src/pages/index` page: the TypeScript type `Pages['index']` comes from it.
#[derive(Serialize, TS)]
struct HomeProps {
    message: String,
}

/// Home page loader: an ordinary axum handler (usual extractors).
#[page("index")]
async fn home() -> Result<HomeProps, Infallible> {
    Ok(HomeProps {
        message: "Hello from Rust!".into(),
    })
}

/// Frontend assets: read from disk in debug, embedded in the binary in release.
#[derive(rust_embed::RustEmbed)]
#[folder = "web/dist/client/"]
#[allow_missing = true]
struct Assets;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    // Pages from `web/src/pages/` + `#[page]` loaders; keeps `web/src/generated/moltn.ts` up to date.
    let pages = moltn_pages::router::<AppState>(moltn_pages::pages!())?;

    let inertia = match std::env::var("MOLTN_VITE_URL") {
        // `moltn dev`: assets and server-side rendering by Vite (hot reload).
        Ok(url) => InertiaConfig::new()
            .root_view(ViteRootView::dev().dev_server(&url){{react_refresh}}.entry("src/app.ts"))
            .ssr(HttpSsrClient::new(format!("{url}/__inertia_ssr"))),
        // Production: bundle and assets built by `moltn build`, inside the binary.
        Err(_) => {
            let manifest = Assets::get(".vite/manifest.json")
                .context("frontend not built: run `moltn build`")?;
            let manifest: ViteManifest = std::str::from_utf8(&manifest.data)?.parse()?;
            let view = ViteRootView::production()
                .manifest(manifest)
                .entry("src/app.ts")
                .asset_base("/build");
            let ssr = EmbeddedSsr::builder(moltn_ssr::embedded_bundle!()).build()?;
            InertiaConfig::new().root_view(view).ssr(ssr)
        }
    };

    let app = pages
        .nest_service(
            "/build",
            EmbeddedAssets::new(|path| Assets::get(path).map(|f| f.data)),
        )
        .layer(InertiaLayer::new(inertia))
        .with_state(AppState);

    // `HOST=0.0.0.0` to accept outside connections (container, server).
    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".into());
    let listener = tokio::net::TcpListener::bind(format!("{host}:{port}")).await?;
    tracing::info!("http://{host}:{port}");
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    /// Every page with parameters has a loader, every loader has a page: `cargo test` says so.
    #[test]
    fn pages_are_valid() {
        moltn_pages::assert_valid::<super::AppState>(moltn_pages::pages!());
    }
}
"#;

const GITIGNORE: &str = "/target\nnode_modules/\nweb/dist/\n";

const TSCONFIG: &str = r#"{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "noEmit": true,
    "skipLibCheck": true,
    "types": ["vite/client"]
  },
  "include": ["src"]
}
"#;

const REACT_TSCONFIG: &str = r#"{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "jsx": "react-jsx",
    "strict": true,
    "noEmit": true,
    "skipLibCheck": true,
    "types": ["vite/client"]
  },
  "include": ["src"]
}
"#;

// --- Vue ---

const VUE_PACKAGE: &str = r#"{
  "name": "{{name}}-web",
  "private": true,
  "type": "module",{{package_manager}}
  "engines": { "node": ">=22" },
  "scripts": {
    "dev": "vite",
    "build": "vite build && vite build --ssr",
    "typecheck": "vue-tsc -p ."
  },
  "dependencies": {
    "@inertiajs/vue3": "^3.11.0",
    "vue": "^3.5.45"
  },
  "devDependencies": {
    "@inertiajs/core": "^3.11.0",
    "@inertiajs/vite": "^3.11.0",
    "@moltn/vite": "{{moltn_vite}}",
    "@vitejs/plugin-vue": "^6.0.10",
    "typescript": "~6.0.3",
    "vite": "^8.3.8",
    "vue-tsc": "^3.3.14"
  }
}
"#;

const VUE_VITE: &str = r#"import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import inertia from '@inertiajs/vite'
import moltn from '@moltn/vite'

export default defineConfig({
  plugins: [vue(), inertia(), moltn()],
})
"#;

const VUE_INDEX: &str = r#"<script setup lang="ts">
import { ref } from 'vue'
import { Head, Link } from '@inertiajs/vue3'
import { route, type Pages } from '../generated/moltn'

// Props typed from the Rust struct `HomeProps` (src/main.rs).
defineProps<Pages['index']>()
const count = ref(0)
</script>

<template>
  <Head title="Home" />
  <h1>{{ message }}</h1>
  <button @click="count++">clicks: {{ count }}</button>
  <p><Link :href="route('about')">About</Link></p>
</template>
"#;

const VUE_ABOUT: &str = r#"<script setup lang="ts">
import { Head, Link } from '@inertiajs/vue3'
import { route } from '../generated/moltn'
</script>

<template>
  <Head title="About" />
  <h1>About</h1>
  <p>A page without a loader: the file is enough, the route follows from its name.</p>
  <Link :href="route('index')">Home</Link>
</template>
"#;

// --- React ---

const REACT_PACKAGE: &str = r#"{
  "name": "{{name}}-web",
  "private": true,
  "type": "module",{{package_manager}}
  "engines": { "node": ">=22" },
  "scripts": {
    "dev": "vite",
    "build": "vite build && vite build --ssr",
    "typecheck": "tsc -p ."
  },
  "dependencies": {
    "@inertiajs/react": "^3.11.0",
    "react": "^19.3.1",
    "react-dom": "^19.3.1"
  },
  "devDependencies": {
    "@inertiajs/core": "^3.11.0",
    "@inertiajs/vite": "^3.11.0",
    "@moltn/vite": "{{moltn_vite}}",
    "@types/react": "^19.3.0",
    "@types/react-dom": "^19.0.0",
    "@vitejs/plugin-react": "^6.3.0",
    "typescript": "^7.0.2",
    "vite": "^8.3.8"
  }
}
"#;

const REACT_VITE: &str = r#"import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import inertia from '@inertiajs/vite'
import moltn from '@moltn/vite'

export default defineConfig({
  plugins: [react(), inertia(), moltn()],
})
"#;

const REACT_INDEX: &str = r#"import { useState } from 'react'
import { Head, Link } from '@inertiajs/react'
import { route, type Pages } from '../generated/moltn'

// Props typed from the Rust struct `HomeProps` (src/main.rs).
export default function Index({ message }: Pages['index']) {
  const [count, setCount] = useState(0)
  return (
    <>
      <Head title="Home" />
      <h1>{message}</h1>
      <button onClick={() => setCount(count + 1)}>clicks: {count}</button>
      <p>
        <Link href={route('about')}>About</Link>
      </p>
    </>
  )
}
"#;

const REACT_ABOUT: &str = r#"import { Head, Link } from '@inertiajs/react'
import { route } from '../generated/moltn'

export default function About() {
  return (
    <>
      <Head title="About" />
      <h1>About</h1>
      <p>A page without a loader: the file is enough, the route follows from its name.</p>
      <Link href={route('index')}>Home</Link>
    </>
  )
}
"#;

// --- Svelte ---

const SVELTE_PACKAGE: &str = r#"{
  "name": "{{name}}-web",
  "private": true,
  "type": "module",{{package_manager}}
  "engines": { "node": ">=22" },
  "scripts": {
    "dev": "vite",
    "build": "vite build && vite build --ssr",
    "typecheck": "svelte-check --tsconfig ./tsconfig.json"
  },
  "dependencies": {
    "@inertiajs/svelte": "^3.11.0",
    "svelte": "^5.57.3"
  },
  "devDependencies": {
    "@inertiajs/core": "^3.11.0",
    "@inertiajs/vite": "^3.11.0",
    "@moltn/vite": "{{moltn_vite}}",
    "@sveltejs/vite-plugin-svelte": "^7.3.1",
    "svelte-check": "^4.7.8",
    "typescript": "~6.0.3",
    "vite": "^8.3.8"
  }
}
"#;

const SVELTE_VITE: &str = r#"import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import inertia from '@inertiajs/vite'
import moltn from '@moltn/vite'

export default defineConfig({
  plugins: [svelte(), inertia(), moltn()],
})
"#;

const SVELTE_INDEX: &str = r#"<script lang="ts">
  import { Link } from '@inertiajs/svelte'
  import { route, type Pages } from '../generated/moltn'

  // Props typed from the Rust struct `HomeProps` (src/main.rs).
  let { message }: Pages['index'] = $props()
  let count = $state(0)
</script>

<svelte:head><title>Home</title></svelte:head>
<h1>{message}</h1>
<button onclick={() => count++}>clicks: {count}</button>
<p><Link href={route('about')}>About</Link></p>
"#;

const SVELTE_ABOUT: &str = r#"<script lang="ts">
  import { Link } from '@inertiajs/svelte'
  import { route } from '../generated/moltn'
</script>

<svelte:head><title>About</title></svelte:head>
<h1>About</h1>
<p>A page without a loader: the file is enough, the route follows from its name.</p>
<Link href={route('index')}>Home</Link>
"#;

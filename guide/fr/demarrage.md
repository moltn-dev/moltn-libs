# Démarrer

## Prérequis

- **Rust** 1.99 ou plus récent ([rustup](https://rustup.rs)).
- **Node.js** 22 ou plus récent, pour le développement et la construction du front.
- **pnpm** (recommandé) ou npm. Si pnpm est installé, `moltn new` l'utilise.

Pour vérifier ce que moltn trouve sur votre machine, lancez `moltn doctor` dans un projet.

## Installer la commande `moltn`

Depuis le dépôt moltn :

```bash
cargo install --path crates/moltn
```

La commande est installée dans `~/.cargo/bin`, déjà présent dans votre `PATH` si vous utilisez rustup.

## Créer un projet

```bash
moltn new mon-app            # Vue par défaut
moltn new mon-app --front react
moltn new mon-app --front svelte
```

`moltn new` écrit le projet, installe les dépendances du front, puis compile l'application une première fois (une à quelques minutes selon la machine). À la fin, le projet est prêt :

```bash
cd mon-app
moltn dev
```

Ouvrez l'adresse affichée par moltn :

```
  ➜ moltn  http://127.0.0.1:3000   (Ctrl+C to stop)
```

## Ce que contient un projet

```
mon-app/
  Cargo.toml
  build.rs                  # lit web/src/pages/ à la compilation
  src/main.rs               # serveur : loaders #[page], configuration
  web/
    package.json
    vite.config.ts          # plugins : vue() | react() | svelte(), inertia(), moltn()
    src/
      app.ts                # createInertiaApp()
      pages/
        index.vue           # → /
        about.vue           # → /about
      generated/
        moltn.ts            # généré : types des props et route()
```

## Le cycle de développement

Avec `moltn dev` lancé :

- modifiez un fichier du front (`.vue`, `.tsx`, `.svelte`, CSS) : la page se met à jour **sans rechargement** ;
- modifiez le code Rust, `Cargo.toml`, ou **ajoutez / supprimez une page** : moltn recompile et relance l'application tout seul ; `web/src/generated/moltn.ts` est mis à jour au redémarrage ;
- en cas d'erreur de compilation, moltn attend la modification suivante ;
- **Ctrl+C** arrête tout, sans laisser de processus derrière.

## Et ensuite

- Ajouter des pages et leurs données : [Pages et loaders](pages.md).
- Typer les props et les liens : [Types et routes](types.md).
- Produire le binaire final : [Production](production.md).

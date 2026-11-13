# Configuration — `moltn.toml`

> Statut : **première tranche faite** (dossiers et `moltn dev`), le reste est encore une proposition.
>
> **Fait** : `[front] dir`, `[pages] dir` et `typescript`, `[dev] port`, `vite_port`, `cargo`, `watch`. Lecture dans `moltn_pages::config` (crate `toml`), partagée par la CLI et par `build::scan_project()`. Le fichier est cherché dans le dossier de départ puis dans ses parents (un seul `moltn.toml` à la racine d'un workspace) ; la CLI se place dans son dossier. Clé inconnue → erreur (`deny_unknown_fields`). `moltn doctor` affiche la configuration effective. `moltn new` écrit un `moltn.toml` commenté : `build.rs` ne peut demander à Cargo de le surveiller que s'il existe.
>
> **Pas fait**, lié aux questions ouvertes ci-dessous : `extensions`, `include` / `exclude` (question 2), `prefix` (question 3), `[toolchain]` (le champ `packageManager` de `package.json` fait déjà ce travail), partage de `entry` / `asset_base` avec le plugin Vite et le runtime (question 1), niveau « variable d'environnement » de la priorité, origine de chaque valeur dans `moltn doctor`, `moltn build -p`.

## Problème

moltn fonctionne sans configuration grâce à des conventions, mais ces conventions sont **figées ou dispersées** :

- certaines valeurs ne sont pas modifiables du tout ;
- d'autres le sont, mais à plusieurs endroits qui doivent rester d'accord à la main (le même réglage écrit dans `vite.config.ts` **et** dans `main.rs`) ;
- rien ne permet d'exclure des fichiers de `pages/`, de changer le dossier du front pour toutes les commandes, ou de dire à `moltn dev` quel paquet lancer dans un workspace.

## Inventaire : ce qui est figé ou dupliqué aujourd'hui

| Valeur | Défaut | Où elle est fixée aujourd'hui | Problème |
|---|---|---|---|
| Dossier du front | `web` | option `--web` de chaque commande ; chemins écrits dans `main.rs`, `build.rs`, `#[folder]` de rust-embed | à répéter partout |
| Dossier des pages | `web/src/pages` | argument de `build::scan` ; **en dur** dans `moltn dev` (surveillance) | `moltn dev` ignore un autre dossier |
| Fichier TypeScript généré | `<parent des pages>/generated/moltn.ts` | convention dans `build::scan` et dans `moltn dev` | non modifiable |
| Extensions de pages | `.vue .tsx .jsx .svelte` | constante `EXTENSIONS` | non modifiable |
| Fichiers ignorés | préfixe `_` ou `.` | règle codée dans le scan | pas d'`exclude` / `include` (tests, brouillons, stories…) |
| Préfixe d'URL des pages | `/` | — | impossible de monter les pages sous `/app` |
| Entrée du front | `src/app.ts` | option `entry` de `@moltn/vite` **et** `ViteRootView::entry()` dans `main.rs` | dupliqué |
| Préfixe des assets | `/build/` | option `assetBase` de `@moltn/vite` **et** `asset_base()` + `nest_service("/build")` dans `main.rs` | dupliqué, une divergence donne des 404 |
| Bundle SSR | `web/dist/ssr/ssr.js` | `embedded_bundle!(chemin)` | à aligner avec `outDir` de Vite |
| Assets construits | `web/dist/client` | `#[folder]` de rust-embed | idem |
| Ports de dev | 3000 / 5173 | `--port`, `--vite-port` | à retaper à chaque lancement |
| Commande de l'application | `cargo run` | en dur dans `moltn dev`, `moltn gen`, `moltn new` | pas de `-p`, `--bin`, `--features` pour un workspace |
| Fichiers surveillés par `moltn dev` | `src/**/*.rs`, `Cargo.toml`, `build.rs`, liste des pages | en dur | un crate voisin du workspace modifié ne relance rien |
| Toolchain JS imposée | détectée | API Rust seulement (`with_package_manager`…) | prévu dans `moltn-toolchain.md`, jamais fait |
| Réglages SSR (workers, délai, mémoire) | voir `moltn-ssr.md` | builder Rust | **reste dans le code** (voir plus bas) |

## Proposition

Un fichier **optionnel** `moltn.toml` à la racine du projet Cargo (à côté de `Cargo.toml`). Sans lui, rien ne change : tous les défauts actuels restent. Chaque clé est facultative.

```toml
[front]
dir = "web"                  # dossier du front, relatif à moltn.toml
entry = "src/app.ts"         # relatif à front.dir
asset_base = "/build/"

[pages]
dir = "src/pages"            # relatif à front.dir
extensions = ["vue", "tsx", "jsx", "svelte"]
include = ["**/*"]           # motifs glob, relatifs à pages.dir
exclude = ["**/*.test.*", "**/*.stories.*", "drafts/**"]
prefix = "/"                 # préfixe d'URL de toutes les pages (ex. "/app")
typescript = "src/generated/moltn.ts"   # relatif à front.dir

[dev]
port = 3000
vite_port = 5173
cargo = ["run", "-p", "mon-app"]        # commande lancée par moltn dev / gen
watch = ["src", "Cargo.toml", "build.rs", "../shared/src"]

[toolchain]
package_manager = "pnpm@10"  # même syntaxe que packageManager
runtime = "node@24"
```

### Règles

- **Priorité** : option de la CLI > variable d'environnement > `moltn.toml` > défaut. `moltn doctor` affiche la configuration effective et l'origine de chaque valeur, comme il le fait déjà pour la toolchain.
- **Clé inconnue ou valeur invalide** : erreur qui nomme la clé et liste les clés valides (une faute de frappe ne doit pas être ignorée en silence).
- **Les conventions de nommage des pages** (`[id]`, `[[tab]]`, `[...slug]`, `(groupe)`, `_ignoré`) **ne deviennent pas configurables** : elles sont le contrat entre le scan Rust et le resolver d'Inertia ; le `_` reste ignoré même avec `include`.
- **Les réglages d'exécution du SSR** (workers, délai, mémoire) **restent dans le code** (`EmbeddedSsr::builder`) : ils relèvent du déploiement, pas du projet ; les mettre dans un fichier lu au démarrage d'un binaire de production ajouterait une dépendance à un fichier absent en production.

### Qui lit le fichier

| Lecteur | Clés | Comment |
|---|---|---|
| CLI (`dev`, `build`, `gen`, `doctor`, `new`) | toutes | lecture directe |
| `build.rs` de l'application | `[front]`, `[pages]` | `moltn_pages::build::scan_project()` (nouveau) lit `moltn.toml` ; `scan(dir)` reste pour qui veut tout passer à la main |
| Application (runtime) | `entry`, `asset_base`, chemins `dist/` | **pas de lecture au démarrage** : `build.rs` grave les valeurs dans la table générée (`OUT_DIR`), et des aides (`moltn_ssr::embedded_bundle!()` sans argument, une future aide pour `ViteRootView`) s'en servent. Supprime les duplications `main.rs` ↔ `vite.config.ts` |
| Plugin `@moltn/vite` | `entry`, `asset_base`, `pages.dir` | à trancher (question 1) |

## Questions ouvertes

1. **Comment le plugin Vite obtient la configuration.** (a) il lit lui-même `moltn.toml` (petit parseur TOML en dépendance, contraire à « aucune dépendance d'exécution lourde ») ; (b) la CLI lui passe les valeurs par variables d'environnement (mais `vite build` lancé sans moltn les perd) ; (c) `build.rs` écrit aussi un `moltn.config.json` que le plugin lit (simple, sans dépendance, mais un fichier généré de plus). Préférence actuelle : **(c)**, ou un champ dédié dans `moltn.ts`.
2. **`include` / `exclude` côté front.** Le resolver de `@inertiajs/vite` (`import.meta.glob('./pages/**/*.vue')`) ne connaît pas les exclusions : une page exclue côté Rust resterait importable côté front. Faut-il que `@moltn/vite` fournisse le glob (option `pages` d'Inertia) ? À vérifier dans le code du plugin Inertia.
3. **Préfixe d'URL** : simple `Router::nest` côté axum, mais `route()` et les liens Inertia doivent le connaître : à intégrer au gabarit de `route()`.
4. **Nom du fichier** : `moltn.toml` seul, ou aussi une table `[package.metadata.moltn]` dans `Cargo.toml` (évite un fichier, mais moins visible et plus verbeux) ?
5. **Monorepos** : plusieurs applications moltn dans un workspace Cargo → un `moltn.toml` par application, et `moltn dev -p <app>` pour choisir.

## Critères d'acceptation (si le jalon est lancé)

- Sans `moltn.toml`, comportement et fichiers générés **identiques** à aujourd'hui (tests existants inchangés).
- Changer `pages.dir` dans `moltn.toml` suffit : scan, surveillance de `moltn dev`, `moltn.ts` et plugin suivent, sans toucher au code.
- `exclude = ["**/*.test.vue"]` : un fichier exclu n'est ni une route, ni dans `moltn.ts`, ni importé par le front.
- `entry` et `asset_base` ne s'écrivent plus qu'à un seul endroit.
- `moltn doctor` affiche la configuration effective et son origine ; une clé inconnue donne une erreur claire.
- Guide utilisateur (fr, en) : page « Configuration ».

# `moltn-toolchain` — piloter Node, Bun, Deno et les gestionnaires de paquets depuis Rust

## Objectif

Une API Rust qui répond à : *« quel runtime et quel gestionnaire de paquets ce projet front attend-il, sont-ils disponibles, et comment lancer install / build / dev ? »* Utilisée par la CLI `moltn`, réutilisable par n'importe quel outil Rust (un cockpit, une app Tauri…).

Rôle limité au **développement et au build**. Une app Moltn en production n'utilise jamais ce crate.

## Ne pas réinventer les gestionnaires de versions

proto (Rust, Bun/Deno/Node + npm/pnpm/Yarn), mise et Volta installent déjà les bonnes versions. `moltn-toolchain` **détecte, résout et délègue** ; il n'installe pas de runtime lui-même dans le premier périmètre.

Point d'attention : depuis Node 25, **Corepack n'est plus livré avec Node** (il s'installe à part via `npm install -g corepack`). Ne pas compter sur lui pour obtenir pnpm ou Yarn.

## 1. Détection

Entrée : le dossier du projet front (défaut `web/`). Les dossiers parents sont aussi lus, jusqu'à la racine du dépôt (premier dossier contenant `.git`) : dans un monorepo, le lockfile et `packageManager` sont souvent à la racine. Le chemin est rendu absolu avant la remontée (bug corrigé en M3 : `--web web` ne remontait pas). Sources lues, par priorité décroissante :

**Gestionnaire de paquets**
1. Configuration explicite : `JsProject::with_package_manager` / `with_runtime` (0.1). `moltn.toml` n'existe pas encore : proposé dans `config.md` (section `[toolchain]`).
2. `package.json` → `packageManager` (`"pnpm@10.4.1+sha512..."`) ; `devEngines.packageManager` s'il est présent.
3. Lockfiles : `pnpm-lock.yaml` → pnpm, `yarn.lock` (+ `.yarnrc.yml` → Yarn moderne) → Yarn, `bun.lock` / `bun.lockb` → bun, `package-lock.json` → npm, `deno.lock` / `deno.json` → deno.
4. Plusieurs lockfiles → avertissement explicite, on prend la source de priorité la plus haute.
5. Rien → npm par défaut.

**Runtime et version**
1. Configuration explicite.
2. `.prototools`, `mise.toml` / `.tool-versions`, `.nvmrc`, `.node-version`.
3. `package.json` → `engines.node`, `devEngines.runtime`.
4. Gestionnaire bun ou deno → runtime bun ou deno.
5. Rien → Node, contrainte « LTS actif minimum » non bloquante (avertissement seulement).

Résultat : une structure sérialisable, utile aussi pour l'affichage (`moltn doctor`).

```rust
pub struct ToolchainSpec {
    pub runtime: Runtime,                  // Node | Bun | Deno
    pub runtime_req: Option<Requirement>,  // contrainte npm (`^20 || >=22`, `24.x`, `lts/*`…)
    pub package_manager: PackageManager,   // Npm | Pnpm | Yarn | Bun | Deno
    pub pm_req: Option<Requirement>,       // `packageManager`, devEngines, .prototools
    pub yarn_berry: bool,
    pub sources: Vec<DetectedFrom>,        // d'où vient chaque décision
    pub warnings: Vec<String>,
    pub prototools: Option<PathBuf>,       // active la délégation à proto
    pub mise_config: Option<PathBuf>,      // active la délégation à mise
}
```

`Requirement` interprète la syntaxe npm (le crate `semver` suit la syntaxe Cargo : une version nue y est un `^`, alors qu'en npm `24.1.0` est exact et `24` vaut `24.x`). Les alias (`lts/*`, `node`) sont gardés tels quels et ne sont pas vérifiés.

## 2. Résolution

Transformer la spec en commandes exécutables, dans cet ordre :
1. **proto** présent et `.prototools` → `proto exec <runtime> <gestionnaire> -- <commande>` (syntaxe vérifiée dans la doc de proto ; les versions viennent de `.prototools`). La sonde de version passe par le même préfixe.
2. **mise** présent et fichier mise / `.tool-versions` → `mise exec -- <commande>`.
3. **Volta** : ses shims dans le `PATH` choisissent la bonne version tout seuls → exécution directe.
4. Sinon, binaires du `PATH` : lire `node --version` (ou bun/deno), comparer à `runtime_req`, idem pour le gestionnaire.
5. Incompatibilité ou absence → **erreur actionnable** : ce qui est attendu, ce qui est trouvé, et la commande exacte à lancer (`proto install node 24`, `mise use node@24`, ou lien d'installation).

Windows : gérer `.cmd` / `.ps1` des shims (`npm.cmd`, `pnpm.cmd`). Utiliser le crate `which`.

La résolution est **bloquante** (elle lance `--version`) : depuis du code async, l'appeler dans `spawn_blocking`.

## 3. Exécution

```rust
let project = JsProject::detect("web")?;          // détection (synchrone, pur)
let tc = project.resolve()?;                       // résolution (lit le PATH, lance --version)

tc.install(InstallOpts { frozen_lockfile: ci }).await?;   // npm ci | pnpm i --frozen-lockfile | yarn --immutable | bun i --frozen-lockfile | deno install
tc.run_script("build", &[]).await?;                         // <pm> run build
let dev = tc.spawn_script("dev", &["--port", "5173"])?;     // processus supervisé
dev.wait_ready(ReadyProbe::Tcp("localhost:5173".into()), Duration::from_secs(60)).await?;
```

- Sortie standard et erreur relayées vers `tracing` (`[<script>] ligne`), ou vers un callback (`Toolchain::with_output`, `DevProcess::spawn_with_output`). `moltn` utilise le callback : `tracing` échappe les codes de couleur des outils lancés.
- `ReadyProbe::Tcp` suffit (une connexion qui aboutit) ; pas de client HTTP.
- `DevProcess` tue le processus **et ses enfants** à la destruction (`Drop`) et sur Ctrl+C : groupes de processus sous Unix (SIGTERM, puis SIGKILL après 2 s), Job Objects « tuer à la fermeture » sous Windows. Sous Windows, même un `moltn` tué brutalement (`taskkill /F`) n'en laisse aucun : le système ferme le job. Sous Unix, un SIGKILL de `moltn` laisse le groupe vivant (pas d'équivalent portable).
- Codes de sortie non nuls → erreur avec les dernières lignes de sortie.
- Deno : `deno task <script>` (Deno 2 lit les scripts de `package.json`) et `deno install`. Non testé en réel (aucun exemple Deno).
- pnpm 11 (constaté au spike) : l'install **échoue** (`ERR_PNPM_IGNORED_BUILDS`) tant qu'un paquet avec script de build n'est ni autorisé ni refusé dans `allowBuilds` (`pnpm-workspace.yaml`). Reconnaître cette erreur et afficher un message actionnable (nom du paquet, ligne à ajouter) ; `moltn new` génère un `allowBuilds` pour les dépendances qu'il installe.

## 4. Build et `build.rs`

**Par défaut, `build.rs` ne lance jamais npm ni Vite** : c'est lent, demande le réseau, casse docs.rs et les builds hors ligne. Le parcours normal :

- `moltn build` = `tc.install(frozen)` → `tc.run_script("build")` (client + SSR) → `cargo build --release`.
- En release, `moltn-ssr` et l'embarquement des assets **échouent à la compilation** si `dist/` est absent, avec un message qui dit de lancer `moltn build`.
- Option explicite (variable `MOLTN_BUILD_FRONT=1` ou feature) pour que `build.rs` déclenche le build front via ce crate, pour ceux qui veulent un `cargo build --release` autonome (CI, Docker).

## 5. API d'inspection

`tc.describe()` renvoie la spec, les binaires résolus (chemins, versions) et le gestionnaire de versions utilisé. Sert à `moltn doctor` et à tout outil qui veut afficher l'état de la toolchain.

## Plus tard (non planifié)

- Installation directe de Node/Bun/Deno (téléchargement officiel + vérification des sommes de contrôle) quand aucun gestionnaire de versions n'est présent. Seulement si des utilisateurs le réclament.
- Cache des `node_modules` entre builds CI.

## Tests

État M3 : 19 tests de détection, 10 de résolution (faux binaires), 3 d'exécution dont l'arrêt d'un petit-enfant, 3 réels `#[ignore]` (script pnpm, échec avec dernières lignes, Vite tué et port libéré). Linux et macOS : compilation et clippy vérifiés par compilation croisée, **pas exécutés**.


- Détection : un dossier fixture par cas (chaque lockfile, `packageManager`, `devEngines`, fichiers de version, conflits).
- Résolution : `PATH` simulé avec de faux binaires qui renvoient une version donnée ; cas proto/mise présents ou absents.
- Exécution réelle (Node, Bun, Deno, chaque gestionnaire) : tests `#[ignore]` lancés dans une matrice CI dédiée.
- Arrêt propre : `DevProcess` détruit → aucun processus Vite restant (Linux, macOS, Windows).

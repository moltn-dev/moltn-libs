# Configuration

moltn fonctionne sans configuration. Un fichier `moltn.toml`, **optionnel**, sert à déclarer une autre disposition des dossiers ou à régler `moltn dev`. Un projet créé par `moltn new` en contient un, entièrement commenté.

## Les clés

Toutes sont facultatives ; voici les valeurs par défaut :

```toml
[front]
dir = "web"                             # dossier du front, relatif à moltn.toml

[pages]
dir = "src/pages"                       # relatif à front.dir
typescript = "src/generated/moltn.ts"   # relatif à front.dir

[dev]
port = 3000
vite_port = 5173
cargo = ["run"]                         # arguments de cargo qui lancent l'application
watch = ["src", "Cargo.toml", "build.rs"]   # ce qui la fait relancer
```

- **`pages.typescript`** : sans cette clé, le fichier suit le dossier des pages (`generated/moltn.ts` à côté de lui).
- **`dev.cargo`** : dans un workspace à plusieurs binaires, `["run", "-p", "mon-app"]`. `moltn gen` utilise la même commande.
- **`dev.watch`** : fichiers et dossiers, relatifs à `moltn.toml`. Un dossier compte pour tous ses fichiers : ajoutez-y vos migrations, vos gabarits, un crate voisin.
- Une option de la ligne de commande (`--port`, `--vite-port`, `--web`) l'emporte sur le fichier.
- Une clé inconnue est une erreur qui la nomme : une faute de frappe n'est jamais ignorée en silence.

`moltn doctor` affiche la configuration effective.

## Où placer le fichier

À côté du `Cargo.toml` de l'application, ou dans un dossier parent. Les commandes `moltn` le cherchent depuis le dossier courant en remontant, et se placent dans son dossier : elles se lancent donc de n'importe quel sous-dossier.

Côté Rust, `build.rs` le lit avec `scan_project()` :

```rust
fn main() {
    if let Err(e) = moltn_pages::build::scan_project() {
        panic!("{e}");
    }
}
```

## Exemple : serveur et front côte à côte

```
mon-app/
├── Cargo.toml        # [workspace] members = ["server"]
├── moltn.toml
├── server/           # crate Rust : Cargo.toml, build.rs, src/, migrations/
└── web/              # front
```

```toml
# moltn.toml
[dev]
watch = ["server/src", "server/migrations", "server/Cargo.toml", "server/build.rs"]
```

Le front est déjà dans `web/` (valeur par défaut). Dans `server/src/main.rs`, les chemins lus à la compilation sont relatifs au crate :

```rust
#[derive(rust_embed::RustEmbed)]
#[folder = "../web/dist/client/"]
struct Assets;

let ssr = EmbeddedSsr::builder(moltn_ssr::embedded_bundle!("../web/dist/ssr/ssr.js")).build()?;
```

## Ce qui ne se configure pas ici

- Les conventions de nommage des pages (`[id]`, `[[tab]]`, `[...slug]`, `(groupe)`, `_ignoré`).
- Les réglages du rendu serveur (nombre de moteurs, délai, mémoire) : ils restent dans le code, voir [Rendu serveur](rendu-serveur.md).
- L'entrée du front et le préfixe des assets : ils s'écrivent encore dans `vite.config.ts` (options de `moltn()`) et dans `main.rs`.

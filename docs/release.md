# Publication 0.1

Préparée en M6 ; **à lancer par le mainteneur** (comptes crates.io et npm, actions publiques irréversibles).

## État vérifié

- Noms libres (vérifié en M6) : `moltn-ssr`, `moltn-pages`, `moltn-pages-macros`, `moltn-toolchain`, `moltn` (CLI ; `moltn-cli` libre aussi) sur crates.io ; `@moltn/vite` sur npm. Organisation npm `moltn` : **non vérifiable automatiquement** (npmjs.com bloque les requêtes), à créer avant la publication du paquet.
- `cargo publish --dry-run --workspace` : les cinq crates s'empaquettent et se recompilent depuis leurs sources empaquetées, dans l'ordre des dépendances.
- `npm pack --dry-run` (`packages/vite`) : 6 fichiers, 3,8 ko ; `prepack` reconstruit `dist/` ; `publishConfig.access = public` (paquet à portée).
- Licences : `LICENSE-MIT`, `LICENSE-APACHE` (texte officiel) à la racine ; champ `license = "MIT OR Apache-2.0"` partout.
- `moltn new` installé depuis crates.io ne trouve pas de dépôt local et écrit des dépendances publiées (`"0.1"`, `^0.1.0`).

## À faire avant de publier

1. **Dépôt public** : fait. `https://github.com/moltn-dev/moltn-libs` ; `repository` et `homepage` hérités de `[workspace.package]`, renseignés dans `packages/vite/package.json`, liens dans les READMEs.
2. **Nom du crate de la CLI** : `moltn` (décidé ; `cargo install moltn`). *À revoir : une application nommée « moltn » est prévue dans la même organisation ; `moltn-cli` était libre.*
3. **Site du guide** : fait. VitePress (`guide/`), déployé sur GitHub Pages par `.github/workflows/docs.yml`, domaine `docs.moltn.eu`. Reste côté mainteneur : Settings → Pages → Source « GitHub Actions », domaine personnalisé, et l'enregistrement DNS `CNAME docs → moltn-dev.github.io`.

## Publier

```bash
cargo login                          # jeton crates.io du mainteneur
cargo publish --workspace            # ordre des dépendances géré par cargo
```

```bash
npm login
cd packages/vite && npm publish      # crée @moltn/vite (organisation npm « moltn » requise)
```

## Vérifier après publication

Hors du dépôt moltn, sur une machine propre :

```bash
cargo install moltn
moltn new essai && cd essai && moltn dev
```

# Publier moltn (crates.io et npm)

À faire par le mainteneur : les deux registres demandent ses comptes, et **une publication ne s'annule pas** (une version publiée ne se remplace pas, un nom ne se libère pas ; on peut seulement masquer une version et en publier une autre).

Ce qui est publié :

| Registre | Paquets |
|---|---|
| crates.io | `moltn-pages-macros`, `moltn-pages`, `moltn-toolchain`, `moltn-ssr`, `moltn-cli` (la commande `moltn`) |
| npm | `@moltn/vite` |

Les deux sont nécessaires : un projet créé par `moltn new` dépend des crates **et** de `@moltn/vite`.

## 0. Avant chaque publication

```bash
git status                                   # arbre propre, tout est commité et poussé
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
pnpm install
pnpm -r build
cargo test --workspace --all-features
pnpm -r test
```

Répétition générale, sans rien envoyer :

```bash
cargo publish --dry-run --workspace
```

```bash
cd packages/vite && npm pack --dry-run
```

**Si la répétition échoue sur `moltn` avec « could not find `config` in `moltn_pages` »** (ou toute erreur qui ne se reproduit pas avec `cargo build`) : cargo réutilise un fichier compilé lors d'une répétition précédente, parce qu'il identifie un crate par son nom et sa version, pas par son contenu. Supprimer les fichiers `target/debug/deps/libmoltn_*` et les dossiers `target/debug/.fingerprint/moltn-*`, ou tout `target/`, puis relancer.

## 1. crates.io

### Une seule fois : le compte

1. Se connecter sur <https://crates.io> avec GitHub.
2. **Vérifier son adresse e-mail** : <https://crates.io/settings/profile>, saisir l'adresse, cliquer sur le lien reçu. Sans cela : `A verified email address is required to publish crates`. Si le message n'arrive pas : regarder les indésirables, puis utiliser « Resend » sur la même page, ou essayer une autre adresse.
3. Créer un jeton : <https://crates.io/settings/tokens> → « New Token ».
   - Scopes : `publish-new` et `publish-update`.
   - Crates : motif `moltn*`.
   - Expiration : 30 ou 90 jours.
4. Enregistrer le jeton sur la machine (la commande le demande ; il va dans `~/.cargo/credentials.toml`) :

```bash
cargo login
```

### Publier

```bash
cargo publish --workspace
```

Cargo publie les cinq crates dans l'ordre de leurs dépendances. Si la commande s'interrompt après en avoir publié une partie, la relancer : les versions déjà en ligne sont ignorées.

### Vérifier

```bash
cargo install moltn-cli
```

```bash
moltn --version
```

La documentation apparaît sur <https://docs.rs/moltn-ssr> quelques minutes plus tard. La feature `v8` n'est pas construite par docs.rs (elle n'est pas dans les features par défaut).

## 2. npm

### Une seule fois : le compte et l'organisation

1. Créer un compte sur <https://www.npmjs.com> et activer la double authentification.
2. Créer l'organisation **`moltn`** (gratuite pour des paquets publics) : c'est elle qui autorise le nom `@moltn/vite`. Si le nom est pris, il faudra changer la portée du paquet (dans `packages/vite/package.json`, les gabarits de `moltn new` et le guide).
3. Se connecter depuis le terminal :

```bash
npm login
```

### Publier

```bash
cd packages/vite
```

```bash
npm publish
```

`prepack` reconstruit `dist/` avant l'envoi ; `publishConfig.access = public` est déjà dans `package.json` (obligatoire pour un paquet à portée).

### Vérifier

```bash
npm view @moltn/vite version
```

## 3. Vérification de bout en bout

Hors du dépôt, dans un dossier vide :

```bash
moltn new essai
```

```bash
cd essai && moltn dev
```

Le `Cargo.toml` du projet doit contenir `moltn-ssr = "0.1"` (et non un chemin local), et `web/package.json` doit contenir `"@moltn/vite": "^0.1.0"`. Tant que le dépôt moltn est présent à l'endroit où la commande a été compilée, `moltn new` préfère les chemins locaux : pour tester la version publiée, installer la commande avec `cargo install moltn-cli` et non `cargo install --path`.

## 4. Version suivante

1. Monter `version` dans chaque `Cargo.toml` de `crates/` (et la version des dépendances internes : `moltn-ssr` dans le `Cargo.toml` racine, `moltn-pages-macros` dans `moltn-pages`, `moltn-pages` et `moltn-toolchain` dans `moltn`), ainsi que dans `packages/vite/package.json`.
2. Mettre à jour les versions écrites par `moltn new` (`crates/moltn-cli/src/main.rs` : `"0.1"`, `^0.1.0`).
3. Refaire les étapes 0 à 3.
4. Poser une étiquette : `git tag v0.2.0 && git push --tags`.

## 5. En cas d'erreur après publication

Masquer une version (elle reste téléchargeable par ceux qui l'ont déjà dans leur `Cargo.lock`, mais n'est plus choisie) :

```bash
cargo yank --version 0.1.0 moltn-ssr
```

Sur npm, une version peut être dépubliée dans les 72 heures (`npm unpublish @moltn/vite@0.1.0`) ; au-delà, la marquer obsolète :

```bash
npm deprecate @moltn/vite@0.1.0 "use 0.1.1"
```

Dans les deux cas, publier ensuite une version corrigée avec un numéro supérieur.

## État (novembre 2026)

- Dépôt : <https://github.com/moltn-dev/moltn-libs> ; `repository` et `homepage` renseignés partout.
- Noms libres sur crates.io au 13 novembre 2026 : `moltn-ssr`, `moltn-pages`, `moltn-pages-macros`, `moltn-toolchain`, `moltn-cli`.
- Répétition `cargo publish --dry-run --workspace` : verte pour les cinq crates.
- Répétition npm : non relancée depuis M6.
- **Nom de la commande (décidé le 13 novembre 2026)** : « moltn » est le nom de l'application de gestion de projet ; le crate `moltn` lui est réservé et **n'est pas publié par ce dépôt**. La commande de cet outillage est le crate `moltn-cli`, dont l'exécutable s'appelle toujours `moltn` (`moltn dev`, `moltn new`…).
- Site du guide : VitePress, déployé par `.github/workflows/docs.yml`. Demande que GitHub Pages soit activé (Settings → Pages → Source « GitHub Actions »). Le site fonctionne avec ou sans domaine personnalisé (`docs.moltn.eu`).

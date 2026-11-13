# Dépannage

Les messages de moltn sont en anglais ; chaque entrée cite le message, puis l'explique.

### `moltn` : commande introuvable

La CLI n'est pas installée, ou `~/.cargo/bin` n'est pas dans le `PATH` :

```bash
cargo install --path crates/moltn   # depuis le dépôt moltn
```

Après une mise à jour du dépôt moltn, relancez la même commande.

### `node 24 required, 20.11.0 found`

La version de Node installée ne correspond pas à celle que le projet demande (`.nvmrc`, `engines`…). Le message donne la commande à lancer (`proto install node 24`, `mise use node@24`, `volta install node@24` ou le lien de téléchargement). `moltn doctor` indique quel fichier impose la version.

### `pnpm blocked the install: unapproved install scripts`

pnpm 11 demande d'autoriser ou de refuser explicitement les scripts d'installation des paquets. Ajoutez, comme indiqué dans le message, dans `pnpm-workspace.yaml` :

```yaml
allowBuilds:
  nom-du-paquet: false   # true si son script est nécessaire
```

### `Node module "fs" is imported by …`

Une page, ou une bibliothèque qu'elle utilise, importe un module Node : il n'existe pas dans le moteur de rendu serveur. Remplacez la bibliothèque, ou ne l'importez que côté navigateur (`onMounted`, `useEffect`, `onMount`). Voir [Rendu serveur](rendu-serveur.md).

### `invalid moltn pages: …`

Pages et loaders ne correspondent pas ; chaque ligne dit quoi corriger. Voir [Pages et loaders](pages.md#vérifications-au-démarrage).

### `… produce the same route …` à la compilation

Deux fichiers de `pages/` donnent la même URL (par exemple `about.vue` et `about/index.vue`). Supprimez ou renommez l'un des deux.

### `moltn.ts is out of date`

`moltn gen --check` a trouvé un fichier en retard sur le code Rust. Lancez `moltn gen` (ou relancez `moltn dev`) et commitez le fichier.

### `Invalid module name in augmentation, module '@inertiajs/core' cannot be found`

Les props partagées demandent `@inertiajs/core` en dépendance directe du front :

```bash
pnpm add -D @inertiajs/core
```

### `vue-tsc` ou `svelte-check` plante avec TypeScript 7

Ces outils ne supportent pas encore TypeScript 7 : gardez `"typescript": "~6.0.3"` dans les projets Vue et Svelte.

### La page s'affiche, mais sans rendu serveur

Le rendu serveur a échoué et le navigateur a pris le relais. La cause est dans les journaux de l'application (`SSR failed, falling back to client-side rendering`, avec le nom de la page et l'erreur). Le plus souvent : un accès à `window` ou `document` pendant le rendu.

### « Frontend not built » alors que le front est construit

En debug, les assets sont lus sur le disque à partir d'un dossier repéré **à la compilation**. Si l'application a été compilée avant le premier build du front (constaté quand le dossier est désigné avec `..`, par exemple `#[folder = "../web/dist/client/"]`), elle ne le retrouve pas ensuite. Recompilez le crate qui déclare les assets (modifiez ou « touchez » son fichier source), puis relancez. `moltn dev` et `moltn build` ne sont pas concernés.

### Le port 3000 (ou 5173) est déjà utilisé

`moltn dev` s'arrête avec `port 3000 is already in use`. Arrêtez l'autre processus, ou changez de ports :

```bash
moltn dev --port 3001 --vite-port 5174
```

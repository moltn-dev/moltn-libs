# Guide moltn

*[English version](../en/)*

moltn sert à écrire une application web dont le **serveur est en Rust** (axum) et l'**interface en Vue, React ou Svelte**, reliés par [Inertia](https://inertiajs.com) : pas d'API JSON à concevoir, le serveur choisit une page et lui passe ses données.

Ce que moltn ajoute :

- **Rendu serveur dans le binaire.** La première visite reçoit du HTML déjà rendu (référencement, affichage immédiat), sans serveur Node à côté : un moteur JavaScript est embarqué dans l'application.
- **Routage par fichiers.** `web/src/pages/users/[id].vue` répond à `/users/42`. La logique serveur d'une page est une fonction Rust annotée `#[page]`.
- **Types de bout en bout.** Les structures Rust deviennent des types TypeScript : renommer un champ côté Rust fait échouer la vérification de types côté front.
- **Une commande pour tout.** `moltn new`, `moltn dev`, `moltn build`.

En production, tout tient dans **un exécutable** : serveur, pages, assets, rendu serveur. Node n'est utile qu'au développement et à la construction du front.

## Sommaire

1. [Démarrer](demarrage.md) : installer, créer un projet, le lancer.
2. [Pages et loaders](pages.md) : routage par fichiers, données serveur, formulaires.
3. [Types et routes](types.md) : props typées, `route()`, props partagées, `moltn gen`.
4. [Rendu serveur](rendu-serveur.md) : fonctionnement, règles pour le code des pages, réglages.
5. [Commandes](cli.md) : référence de la CLI.
6. [Production](production.md) : construire et déployer le binaire.
7. [Avec Loco](loco.md) : brancher moltn dans une application Loco.
8. [Dépannage](depannage.md) : messages d'erreur courants et solutions.

> **État** : moltn est en développement (0.1) et n'est pas encore publié sur crates.io ni npm. Les projets créés par `moltn new` dépendent du dépôt moltn présent sur la machine.

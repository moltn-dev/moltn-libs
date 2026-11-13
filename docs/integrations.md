# Intégrations

Moltn n'est pas un framework : ce sont des briques. Les intégrations ci-dessous sont des exemples et des petits adaptateurs, pas des forks.

## axum + `veer` (cible principale)

Le parcours de référence, utilisé par tous les exemples :
- `veer` : protocole Inertia v3, layer axum, gabarit racine, manifeste Vite, props différées et fusionnées.
- `moltn-ssr` : implémente `veer::SsrClient`.
- `moltn-pages` : produit un `axum::Router` à fusionner avec les routes de l'application.

Toute lacune de `veer` rencontrée (hook manquant, comportement v3 non couvert) : **ouvrir une issue ou une PR chez `veer`** plutôt que de dupliquer la logique dans Moltn. Le noter dans `docs/upstream.md`.

## Loco

Loco est construit sur axum (routage) et Sea-ORM. L'intégration passe donc par le même chemin qu'axum :
- exemple `examples/loco-vue` : contrôleurs Loco existants + pages Moltn fusionnées dans le routeur de Loco ;
- état de l'application Loco (`AppContext`) accessible dans les loaders via les extracteurs habituels ;
- à évaluer : un initializer Loco qui branche `veer` + `moltn-ssr` en une ligne.

**État M6 (Loco 1.3.1, axum 0.8 comme moltn) : fait, sans code d'adaptation.** Trois points d'accroche du trait `Hooks` suffisent :
- `before_routes` renvoie `moltn_pages::router::<AppContext>(pages())` : les pages deviennent le routeur de départ de Loco, les loaders lisent `State<AppContext>` ;
- `routes` garde les contrôleurs Loco (ici le POST du formulaire) et les routes par défaut (`/_ping`, `/_health`) ;
- `after_routes` ajoute les assets (`/build`) et `veer::InertiaLayer`.

Sans les features `with-db` ni `cli` de Loco, l'application démarre par `cargo run` (un `main` de 10 lignes appelle `boot` puis `start`) : `moltn dev` fonctionne tel quel (port par `PORT`). Six tests de conformité (rendu avec l'état Loco, visite Inertia, contrôleur, routes Loco, liens, assets). Initializer « une ligne » : pas fait, le gain est faible face à ces trois méthodes, à reconsidérer sur demande.

## Ferro

Ferro, framework inspiré de Laravel, a sa propre intégration Inertia (`ferro-inertia`, couplé aux types de requête et de réponse de Ferro) et n'est pas construit sur les types axum de la même façon.
- Étape 1 : vérifier si Ferro expose un point d'accroche pour le SSR. Si oui, fournir un adaptateur qui y branche `moltn-ssr` (le moteur ne dépend pas de `veer`, seule l'impl du trait en dépend).
- Étape 2 : proposer la contribution aux mainteneurs de Ferro.
- `moltn-pages` n'est pas visé pour Ferro au départ (son routeur est différent).

**Évaluation M6 (ferro-rs / ferro-inertia 0.3.10) : pas d'adaptateur possible aujourd'hui.**
1. **Format de page Inertia ≤ v2.** `ferro-inertia` place la page dans un attribut (`<div id="app" data-page="…">`, gabarit par défaut et exemple de gabarit personnalisé). Le client Inertia v3 ne lit que `<script data-page="app" type="application/json">` (`getInitialPageFromDOM`, `@inertiajs/core` 3.11) ; le rendu SSR v3 produit aussi ce format. moltn étant limité à Inertia v3, une page Ferro et un rendu moltn ne sont pas compatibles. (Version du client Inertia livrée par les gabarits front de Ferro : non vérifiée.)
2. **Aucun point d'accroche SSR.** `Inertia::render(req, composant, props)` est synchrone et renvoie la réponse HTML complète ; le HTML est assemblé par remplacement de texte (`{page}`, `{csrf}`). Un rendu serveur, forcément asynchrone, n'a pas de place.
3. **Pas d'axum.** Ferro repose directement sur hyper : `moltn-pages` ne s'applique pas.

Contournement écarté : réécrire le HTML de Ferro après coup (extraire `data-page`, rendre, réinjecter). Fragile, et inutile tant que le format reste v2.

Proposition aux mainteneurs (texte prêt dans `upstream.md`, **non envoyée**) : passer au format v3 et ajouter un trait de rendu serveur asynchrone dans `InertiaConfig`. `moltn-ssr` s'y brancherait sans dépendre de `veer` (`EmbeddedSsr::render(&page)` est indépendant du framework).

## Autres
- **actix-web, Rocket, Poem** : pas avant une demande réelle. Le moteur (`moltn-ssr` sans la feature `veer`) reste utilisable partout via son API directe `render()`.
- **Tauri** : hors périmètre (pas de SSR dans une webview locale). Seul `moltn-toolchain` peut y servir.

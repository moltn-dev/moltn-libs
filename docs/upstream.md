# Points remontés aux projets amont

À signaler aux mainteneurs plutôt qu'à contourner dans moltn (voir `integrations.md`).

## `veer` 0.3.0

- **Double `<title>` avec le SSR.** `ViteRootView` (et `MinimalRootView`) émet toujours `<title>{title}</title>`, même vide, avant les éléments `head` du SSR. Le commentaire du code dit que le titre SSR « gagne », mais un navigateur retient le **premier** `<title>` : sans JavaScript (robots), la page porte le titre statique, et le titre de la page n'apparaît qu'après hydratation. Proposition : ne pas émettre le `<title>` statique quand le SSR fournit un `<title>`, ou ne rien émettre quand `title` est vide. *Statut : à ouvrir.*

- **Avertissement à chaque démarrage en dev.** Pendant l'échauffement du graphe de modules, le point d'accès `/__inertia_ssr` de `@inertiajs/vite` répond `null`. `HttpSsrClient` le traite comme une erreur de décodage : `WARN SSR failed; falling back to client render error=Decode(…)` sur les une ou deux premières requêtes de chaque `moltn dev` (constaté sur Aniplex). Le repli client est le bon comportement ; c'est le niveau du message qui inquiète pour rien. Proposition : traiter un corps `null` comme « pas de rendu » sans avertissement (ou en `debug`). *Statut : à ouvrir.*

## Ferro (`ferro-inertia` 0.3.10)

*Statut : brouillon, non envoyé (à ouvrir par le mainteneur de moltn).*

> **Inertia v3 page format and a server-side rendering hook**
>
> `ferro-inertia` writes the initial page into a `data-page` attribute (`<div id="app" data-page="…">`). The Inertia v3 client only reads `<script data-page="app" type="application/json">…</script>` (see `getInitialPageFromDOM` in `@inertiajs/core` 3.x), which is also what v3 server-side rendering produces.
>
> Would you consider:
> 1. emitting the v3 format (behind a config flag if v2 clients must keep working);
> 2. an optional async SSR hook on `InertiaConfig`, e.g. `trait SsrRenderer { async fn render(&self, page: &serde_json::Value) -> Result<SsrOutput, Error>; }` with `SsrOutput { head: Vec<String>, body: String }`, falling back to client rendering on error.
>
> With (2), embedded renderers such as moltn-ssr (QuickJS inside the Rust process, no Node server) or an HTTP client to Inertia's Node SSR server could plug in without Ferro depending on either.

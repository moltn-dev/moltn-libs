// Remplace `@inertiajs/<framework>/server` dans le build SSR : au lieu d'ouvrir un serveur
// HTTP Node, la fonction de rendu est exposée au moteur JS embarqué côté Rust (moltn-ssr).

// URL / URLSearchParams : absents du moteur, utilisés par <Link> d'Inertia pendant le rendu.
import 'core-js/actual/url'
import 'core-js/actual/url-search-params'

/** Render function passed by Inertia: page object → `{ head, body }`. */
export type Render = (page: unknown) => Promise<{ head: string[]; body: string }>

declare global {
  var __moltnRender: Render | undefined
}

/** Same signature as Inertia's `createServer`; options (port, cluster…) are ignored. */
export default function createServer(render: Render, _options?: unknown): Render {
  globalThis.__moltnRender = render
  return render
}

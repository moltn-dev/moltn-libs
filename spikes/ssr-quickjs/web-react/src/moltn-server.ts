// URL/URLSearchParams : absents de QuickJS, utilisés par <Link> pendant le rendu.
import 'core-js/actual/url'
import 'core-js/actual/url-search-params'

// Remplace @inertiajs/vue3/server (serveur HTTP Node) par un simple enregistrement global,
// appelé par le moteur JS embarqué côté Rust.
type Render = (page: unknown) => Promise<{ head: string[]; body: string }>

export default function createServer(render: Render): Render {
  ;(globalThis as any).__moltnRender = render
  return render
}

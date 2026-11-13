import { builtinModules } from 'node:module'
import { posix } from 'node:path'
import { fileURLToPath } from 'node:url'
import type { Plugin } from 'vite'

/** Options of `moltn()`. */
export interface MoltnOptions {
  /** Application entry, shared by the client and SSR builds. Default: `src/app.ts`. */
  entry?: string
  /** Prefix under which the Rust application serves the assets (`asset_base` of `ViteRootView`). Default: `/build/`. */
  assetBase?: string
}

const INERTIA_SERVER_MODULES = ['@inertiajs/vue3/server', '@inertiajs/react/server', '@inertiajs/svelte/server']
const SERVER_SHIM = fileURLToPath(new URL('./server.js', import.meta.url))
const SSR_CONDITIONS = ['worker', 'production']
const NODE_BUILTINS = new Set(builtinModules.flatMap((m) => [m, `node:${m}`]))
const UNAVAILABLE_NODE_MODULE = '\0moltn-node-unavailable:'

/**
 * Complements `@inertiajs/vite` so that the SSR bundle runs in moltn's embedded
 * JavaScript engine: a single `dist/ssr/ssr.js` file, with no imports and no Node modules.
 *
 * ```ts
 * plugins: [vue(), inertia(), moltn()]
 * ```
 */
export default function moltn(options: MoltnOptions = {}): Plugin {
  const entry = options.entry ?? 'src/app.ts'
  const assetBase = options.assetBase ?? '/build/'
  let ssrBuild = false
  let root = process.cwd()

  return {
    name: '@moltn/vite',

    config(config, env) {
      ssrBuild = Boolean(env.isSsrBuild)
      const input = config.build?.rolldownOptions?.input ?? config.build?.rollupOptions?.input
      if (!ssrBuild) {
        return {
          // Seulement au build : le serveur de dev sert à la racine.
          base: env.command === 'build' ? (config.base ?? assetBase) : undefined,
          // En dev, la page vient de l'application Rust : les URL d'assets (CSS, images)
          // doivent viser le serveur Vite, pas l'origine de la page.
          server:
            env.command === 'serve'
              ? { origin: config.server?.origin ?? `http://localhost:${config.server?.port ?? 5173}` }
              : undefined,
          build: {
            outDir: config.build?.outDir ?? 'dist/client',
            manifest: config.build?.manifest ?? true,
            rolldownOptions: input ? undefined : { input: entry },
          },
        }
      }
      return {
        resolve: { alias: INERTIA_SERVER_MODULES.map((find) => ({ find, replacement: SERVER_SHIM })) },
        ssr: {
          noExternal: true,
          target: 'webworker',
          // « worker » et non « browser » : Svelte y sert son runtime serveur.
          resolve: { conditions: SSR_CONDITIONS, externalConditions: SSR_CONDITIONS },
        },
        define: { 'process.env.NODE_ENV': '"production"' },
        build: {
          outDir: config.build?.outDir ?? 'dist/ssr',
          sourcemap: false,
          rolldownOptions: { output: { entryFileNames: 'ssr.js', codeSplitting: false } },
        },
      }
    },

    configResolved(config) {
      root = config.root
    },

    // Avant tout autre résolveur : un module Node ne doit jamais être remplacé en silence.
    resolveId: {
      order: 'pre',
      handler(source, importer, options) {
        if (!ssrBuild || !NODE_BUILTINS.has(source)) return null
        // `import()` dynamique : dépendance optionnelle (ex. Svelte et node:async_hooks).
        // Le module de remplacement fait échouer l'import, comme dans un navigateur.
        if ((options as { kind?: string }).kind === 'dynamic-import') {
          this.info(`optional import of Node module "${source}" by ${importer ? describe(importer, root) : 'a module'}: it will fail in the embedded SSR engine.`)
          return UNAVAILABLE_NODE_MODULE + source
        }
        this.error(nodeModuleMessage(source, importer, root))
      },
    },

    load(id) {
      if (!id.startsWith(UNAVAILABLE_NODE_MODULE)) return null
      const source = id.slice(UNAVAILABLE_NODE_MODULE.length)
      return `throw new Error(${JSON.stringify(`Node module "${source}" is not available in the embedded SSR engine`)})`
    },

    generateBundle(_, bundle) {
      if (!ssrBuild) return
      const chunks = Object.values(bundle).filter((file) => file.type === 'chunk')
      if (chunks.length !== 1) {
        this.error(`SSR bundle split into ${chunks.length} files; moltn expects a single file (${chunks.map((c) => c.fileName).join(', ')}).`)
      }
      // Les imports dynamiques inlinés (pages chargées à la demande) pointent vers le fichier lui-même.
      const imports = chunks.flatMap((c) => [...c.imports, ...c.dynamicImports].filter((i) => i !== c.fileName))
      if (imports.length > 0) {
        this.error(`the SSR bundle still imports modules (${imports.join(', ')}); the embedded engine loads none.`)
      }
    },
  }
}

function nodeModuleMessage(source: string, importer: string | undefined, root: string): string {
  return [
    `Node module "${source}" is imported by ${importer ? describe(importer, root) : 'an unknown module'}.`,
    'moltn renders on the server in an embedded JavaScript engine, without Node: pages and their dependencies',
    'must not use any Node module (fs, path, stream, crypto…). Replace this package, or import it',
    'in the browser only (onMounted / useEffect / onMount).',
  ].join('\n')
}

/** "package x (path)" for a dependency, "file src/…" for the project. */
function describe(id: string, root: string): string {
  const path = id.split('?')[0]!.replace(/\\/g, '/')
  const pkg = path.match(/.*node_modules\/((?:@[^/]+\/)?[^/]+)/)?.[1]
  if (pkg) return `package ${pkg}\n  (${path})`
  return `file ${posix.relative(root.replace(/\\/g, '/'), path)}`
}

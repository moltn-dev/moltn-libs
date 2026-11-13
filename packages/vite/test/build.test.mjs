// Tests du plugin sur de vrais builds Vite (node --test, après `pnpm build`).
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtemp, mkdir, writeFile, readFile, readdir } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { pathToFileURL } from 'node:url'
import { build } from 'vite'
import moltn from '../dist/index.js'

/** Projet jetable : { chemin relatif: contenu }. */
async function project(files) {
  const root = await mkdtemp(join(tmpdir(), 'moltn-vite-'))
  for (const [path, content] of Object.entries(files)) {
    await mkdir(dirname(join(root, path)), { recursive: true })
    await writeFile(join(root, path), content)
  }
  return root
}

function ssrBuild(root) {
  return build({ root, configFile: false, logLevel: 'silent', plugins: [moltn()], build: { ssr: 'app.js' } })
}

const fakePackage = (name, code) => ({
  [`node_modules/${name}/package.json`]: JSON.stringify({ name, type: 'module', main: 'index.js' }),
  [`node_modules/${name}/index.js`]: code,
})

test('static import of a Node module in the project: fails naming the file', async () => {
  const root = await project({ 'app.js': "import { readFileSync } from 'node:fs'\nexport default readFileSync\n" })
  await assert.rejects(ssrBuild(root), (e) => {
    assert.match(e.message, /Node module "node:fs"/)
    // Chemin relatif au projet, sans le suffixe de requête des plugins (ex. `?vue&type=script`).
    assert.match(e.message, /imported by file app\.js\.\n/)
    return true
  })
})

test('static import of a Node module by a package: fails naming the package', async () => {
  const root = await project({
    'app.js': "import read from 'lit-fichiers'\nexport default read\n",
    ...fakePackage('lit-fichiers', "import fs from 'fs'\nexport default fs.readFileSync\n"),
  })
  await assert.rejects(ssrBuild(root), (e) => {
    assert.match(e.message, /Node module "fs" is imported by package lit-fichiers/)
    return true
  })
})

test('optional dynamic import: the build passes, the import fails at runtime', async () => {
  const root = await project({
    'app.js': "export default () => import('node:async_hooks').then(() => 'loaded', () => 'unavailable')\n",
  })
  await ssrBuild(root)
  const code = await readFile(join(root, 'dist/ssr/ssr.js'), 'utf8')
  assert.doesNotMatch(code, /from\s*["']node:/)
  const { default: load } = await import(pathToFileURL(join(root, 'dist/ssr/ssr.js')).href)
  assert.equal(await load(), 'unavailable')
})

test('Inertia createServer replaced: one file, no imports, defines __moltnRender', async () => {
  const root = await project({
    'app.js': [
      "import createServer from '@inertiajs/vue3/server'",
      "const page = () => import('./page.js')",
      'createServer(async (p) => ({ head: [], body: (await page()).default(p) }))',
      '',
    ].join('\n'),
    'page.js': "export default (p) => `<p>${new URL(p.url, 'http://localhost').pathname}</p>`\n",
  })
  await ssrBuild(root)
  assert.deepEqual(await readdir(join(root, 'dist/ssr')), ['ssr.js'])
  const code = await readFile(join(root, 'dist/ssr/ssr.js'), 'utf8')
  assert.doesNotMatch(code, /^import /m)
  await import(pathToFileURL(join(root, 'dist/ssr/ssr.js')).href)
  const out = await globalThis.__moltnRender({ url: '/users/42' })
  assert.deepEqual(out, { head: [], body: '<p>/users/42</p>' })
})

# @moltn/vite

Vite plugin for [moltn](https://crates.io/crates/moltn): builds an Inertia v3 server-side rendering bundle that runs **inside a Rust binary** (embedded JavaScript engine, no Node server).

```ts
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import inertia from '@inertiajs/vite'
import moltn from '@moltn/vite'

export default defineConfig({
  plugins: [vue(), inertia(), moltn()],
})
```

- One self-contained `dist/ssr/ssr.js`, no imports; Inertia's `createServer` is swapped automatically, no `ssr.ts` to write.
- A static import of a Node module in a page fails the build with the file or package to blame.
- Works the same with React and Svelte.

## Part of moltn

Rust + Inertia v3 (Vue, React, Svelte) with server-side rendering inside the binary.
Guide (English and French): <https://docs.moltn.eu>. Source: <https://github.com/moltn-dev/moltn-libs>.

## License

MIT OR Apache-2.0.

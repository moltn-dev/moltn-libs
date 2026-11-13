# moltn guide

Sources of the user guide published at <https://docs.moltn.eu>: `en/` (served at the site root) and `fr/` (served under `/fr/`). Keep both languages in sync.

```bash
pnpm --filter moltn-guide dev      # local preview with hot reload
pnpm --filter moltn-guide build    # static site in guide/.vitepress/dist
```

The site is deployed to GitHub Pages by `.github/workflows/docs.yml` on every push to `main` that touches `guide/`.

import { defineConfig } from 'vitepress'

const REPO = 'https://github.com/moltn-dev/moltn-libs'

// Les sources restent dans `en/` et `fr/`. L'anglais est servi à la racine du site,
// le français sous `/fr/` ; `README.md` de chaque langue en est la page d'accueil.
// Une page porte le même nom de fichier dans les deux langues : c'est ce qui permet au
// sélecteur de langue d'arriver sur la page équivalente.
export default defineConfig({
  // `/` avec le domaine personnalisé, `/moltn-libs/` sur moltn-dev.github.io : donné par le
  // workflow de déploiement, pour que le site fonctionne dans les deux cas.
  base: process.env.DOCS_BASE || '/',
  title: 'moltn',
  description: 'Rust + Inertia, with server-side rendering, shipped as a single binary.',
  // Liens du dépôt (README.md à la racine du guide) : pas des pages du site.
  srcExclude: ['README.md'],
  rewrites: {
    'en/README.md': 'index.md',
    'en/:page': ':page',
    'fr/README.md': 'fr/index.md',
  },
  lastUpdated: true,
  themeConfig: {
    search: { provider: 'local' },
    socialLinks: [{ icon: 'github', link: REPO }],
    editLink: { pattern: `${REPO}/edit/main/guide/:path` },
  },
  locales: {
    root: {
      label: 'English',
      lang: 'en',
      themeConfig: {
        nav: [
          { text: 'Guide', link: '/getting-started' },
          { text: 'Commands', link: '/cli' },
        ],
        sidebar: [
          {
            text: 'Guide',
            items: [
              { text: 'Introduction', link: '/' },
              { text: 'Getting started', link: '/getting-started' },
              { text: 'Pages and loaders', link: '/pages' },
              { text: 'Types and routes', link: '/types' },
              { text: 'Server-side rendering', link: '/server-rendering' },
            ],
          },
          {
            text: 'Reference',
            items: [
              { text: 'Commands', link: '/cli' },
              { text: 'Configuration', link: '/configuration' },
              { text: 'Production', link: '/production' },
              { text: 'With Loco', link: '/loco' },
              { text: 'Troubleshooting', link: '/troubleshooting' },
            ],
          },
        ],
      },
    },
    fr: {
      label: 'Français',
      lang: 'fr',
      description: 'Rust + Inertia, rendu serveur compris, livré en un seul binaire.',
      themeConfig: {
        nav: [
          { text: 'Guide', link: '/fr/getting-started' },
          { text: 'Commandes', link: '/fr/cli' },
        ],
        sidebar: [
          {
            text: 'Guide',
            items: [
              { text: 'Présentation', link: '/fr/' },
              { text: 'Démarrer', link: '/fr/getting-started' },
              { text: 'Pages et loaders', link: '/fr/pages' },
              { text: 'Types et routes', link: '/fr/types' },
              { text: 'Rendu serveur', link: '/fr/server-rendering' },
            ],
          },
          {
            text: 'Référence',
            items: [
              { text: 'Commandes', link: '/fr/cli' },
              { text: 'Configuration', link: '/fr/configuration' },
              { text: 'Production', link: '/fr/production' },
              { text: 'Avec Loco', link: '/fr/loco' },
              { text: 'Dépannage', link: '/fr/troubleshooting' },
            ],
          },
        ],
        outline: { label: 'Sur cette page' },
        docFooter: { prev: 'Page précédente', next: 'Page suivante' },
        editLink: { pattern: `${REPO}/edit/main/guide/:path`, text: 'Modifier cette page sur GitHub' },
        lastUpdated: { text: 'Mis à jour le' },
        langMenuLabel: 'Changer de langue',
        returnToTopLabel: 'Retour en haut',
        sidebarMenuLabel: 'Menu',
        darkModeSwitchLabel: 'Apparence',
      },
    },
  },
})

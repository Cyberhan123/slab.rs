import tailwindcss from '@tailwindcss/vite'
import { defineConfig } from 'vitepress'

export default defineConfig({
  lang: 'en-US',
  title: 'Slab',
  titleTemplate: ':title | Slab',
  description: 'Local-first AI workspaces for chat, speech, image generation, and shared runtime contracts.',
  cleanUrls: true,
  lastUpdated: true,
  sitemap: {
    hostname: 'https://slab.reorgix.com',
  },
  head: [
    ['meta', { name: 'theme-color', content: '#0d9488' }],
    ['meta', { name: 'color-scheme', content: 'light dark' }],
    ['link', { rel: 'preconnect', href: 'https://fonts.googleapis.com' }],
    ['link', { rel: 'preconnect', href: 'https://fonts.gstatic.com', crossorigin: '' }],
    [
      'link',
      {
        rel: 'stylesheet',
        href: 'https://fonts.googleapis.com/css2?family=Fira+Code:wght@400;500;600&family=Outfit:wght@400;500;600;700&display=swap',
      },
    ],
  ],
  vite: {
    plugins: [tailwindcss()],
  },
  themeConfig: {
    logo: {
      src: '/slab-mark.svg',
      alt: 'Slab',
    },
    nav: [
      { text: 'Guide', link: '/guide/getting-started' },
      { text: 'Reference', link: '/reference/' },
      { text: 'GitHub', link: 'https://github.com/Cyberhan123/slab.rs' },
    ],
    sidebar: {
      '/guide/': [
        {
          text: 'Guide',
          items: [{ text: 'Getting Started', link: '/guide/getting-started' }],
        },
      ],
      '/reference/': [
        {
          text: 'Reference',
          items: [
            { text: 'Overview', link: '/reference/' },
            { text: 'Model Manifest Schema', link: '/reference/model-manifests' },
            { text: 'Settings Document Schema', link: '/reference/settings-document' },
          ],
        },
      ],
    },
    search: {
      provider: 'local',
    },
    socialLinks: [{ icon: 'github', link: 'https://github.com/Cyberhan123/slab.rs' }],
    outline: {
      level: [2, 3],
      label: 'On This Page',
    },
    docFooter: {
      prev: 'Previous',
      next: 'Next',
    },
    footer: {
      message: 'Local-first machine learning workspaces with stable runtime contracts.',
      copyright: 'Copyright (c) Cyberhan123 and contributors.',
    },
  },
})

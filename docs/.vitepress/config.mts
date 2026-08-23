import { defineConfig } from 'vitepress';

export default defineConfig({
  base: '/NexusKVM/',
  title: 'NexusKVM',
  description: 'Official NexusKVM Documentation - Modern Software KVM for Linux with Tauri 2, Rust, and rkvm 0.6.1 fork',
  lang: 'en-US',
  lastUpdated: true,
  cleanUrls: true,

  head: [
    ['link', { rel: 'icon', type: 'image/svg+xml', href: '/logo.svg' }],
    ['meta', { name: 'theme-color', content: '#0ea5e9' }],
    ['meta', { name: 'og:title', content: 'NexusKVM - Official Documentation' }],
    ['meta', { name: 'og:description', content: 'Ultra-low latency software KVM for Linux with Wayland, libei, TLS 1.3 encryption, and rkvm fork.' }],
  ],

  themeConfig: {
    logo: '/logo.svg',
    siteTitle: 'NexusKVM',

    nav: [
      { text: 'Home', link: '/' },
      { text: 'Guide', link: '/guide/getting-started' },
      { text: 'Permissions & Security', link: '/guide/permissions-and-security' },
      { text: 'Architecture', link: '/architecture/overview' },
      { text: 'rkvm Fork', link: '/architecture/rkvm-integration' },
      { text: 'CLI Reference', link: '/reference/nexusctl' },
    ],

    sidebar: [
      {
        text: '🚀 Getting Started',
        items: [
          { text: 'Introduction to NexusKVM', link: '/guide/getting-started' },
          { text: 'Installation & Packages', link: '/guide/installation' },
          { text: 'Configuration & Pairing', link: '/guide/configuration' },
        ],
      },
      {
        text: '🛡️ Security, Network & Permissions',
        items: [
          { text: 'Linux Permissions (/dev/uinput, udev)', link: '/guide/permissions-and-security' },
          { text: 'Network, Firewall & TLS 1.3', link: '/guide/firewall-and-network' },
          { text: 'systemd Services (GDM & Boot)', link: '/guide/systemd-services' },
        ],
      },
      {
        text: '⚙️ Architecture & Integrations',
        items: [
          { text: 'Process Architecture Overview', link: '/architecture/overview' },
          { text: 'rkvm Fork (0.6.1) & Citations', link: '/architecture/rkvm-integration' },
          { text: 'Wayland, EIS & Portals', link: '/architecture/wayland-and-eis' },
          { text: 'Barrier Engine & Hysteresis', link: '/architecture/barrier-engine' },
        ],
      },
      {
        text: '📖 Reference & Help',
        items: [
          { text: 'CLI Reference (nexusctl)', link: '/reference/nexusctl' },
          { text: 'Configuration File Reference', link: '/reference/config-reference' },
          { text: 'Troubleshooting & Diagnostics', link: '/reference/troubleshooting' },
          { text: 'Frequently Asked Questions (FAQ)', link: '/reference/faq' },
        ],
      },
    ],

    search: {
      provider: 'local',
      options: {
        translations: {
          button: {
            buttonText: 'Search documentation',
            buttonAriaLabel: 'Search documentation',
          },
          modal: {
            noResultsText: 'No results found for',
            resetButtonTitle: 'Clear search',
            footer: {
              selectText: 'to select',
              navigateText: 'to navigate',
              closeText: 'to close',
            },
          },
        },
      },
    },

    socialLinks: [
      { icon: 'github', link: 'https://github.com/jigonzalez930209/NexusKVM' },
    ],

    footer: {
      message: 'Released under the MIT License. Built with VitePress & Rust.',
      copyright: 'NexusKVM • Based on the original rkvm work by Florian Larysch',
    },

    outline: {
      level: [2, 3],
      label: 'On this page',
    },

    docFooter: {
      prev: 'Previous page',
      next: 'Next page',
    },
  },
});

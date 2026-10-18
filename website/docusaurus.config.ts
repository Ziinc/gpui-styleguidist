import { themes as prismThemes } from 'prism-react-renderer';
import type { Config } from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';

const repo = 'https://github.com/Ziinc/gpui-styleguidist';

const config: Config = {
  title: 'gpui-styleguidist',
  tagline:
    'Write one example per GPUI component. Get a live web playground, previews on docs.rs, copy-pasteable code and visual regression tests from it.',
  favicon: 'img/favicon.svg',
  url: 'https://gpui-styleguidist.tznc.net',
  baseUrl: '/',
  trailingSlash: false,
  onBrokenLinks: 'throw',
  // The README headings come from an imported partial, which the anchor
  // checker doesn't see.
  onBrokenAnchors: 'ignore',
  markdown: { format: 'detect' },
  presets: [
    [
      'classic',
      {
        docs: false,
        blog: false,
        theme: { customCss: './src/css/custom.css' },
      } satisfies Preset.Options,
    ],
  ],
  stylesheets: [
    'https://fonts.googleapis.com/css2?family=IBM+Plex+Sans:wght@400;500;600;700&family=JetBrains+Mono:wght@400;600&display=swap',
  ],
  themeConfig: {
    colorMode: { respectPrefersColorScheme: true },
    navbar: {
      title: 'gpui-styleguidist',
      logo: { alt: '', src: 'img/favicon.svg' },
      items: [
        { href: '/#get-started', label: 'Get started', position: 'left' },
        { href: '/#writing-examples', label: 'Writing examples', position: 'left' },
        // `pathname://` leaves the link to the browser: /demo is the wasm site,
        // not a Docusaurus route.
        { href: 'pathname:///demo/', label: 'Live demo', position: 'right', target: '_self', className: 'navbar-demo-link' },
        { href: 'https://docs.rs/gpui-styleguidist', label: 'docs.rs', position: 'right' },
        { href: repo, label: 'GitHub', position: 'right' },
      ],
    },
    footer: {
      style: 'light',
      copyright: `Apache-2.0 · <a href="${repo}">GitHub</a> · <a href="https://crates.io/crates/gpui-styleguidist">crates.io</a>`,
    },
    tableOfContents: { minHeadingLevel: 2, maxHeadingLevel: 2 },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.vsDark,
      additionalLanguages: ['rust', 'toml', 'bash'],
    },
  } satisfies Preset.ThemeConfig,
};

export default config;

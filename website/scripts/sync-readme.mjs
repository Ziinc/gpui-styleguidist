// Copies the repository README into the site so the landing page and GitHub
// show the same documentation. The hero renders the README's headline, GIF
// and caption itself, so the body starts after them.
import { copyFileSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const site = join(dirname(fileURLToPath(import.meta.url)), '..');
const root = join(site, '..');
const repo = 'https://github.com/Ziinc/gpui-styleguidist';

const readme = readFileSync(join(root, 'README.md'), 'utf8');
const paragraphs = readme.split(/\n{2,}/);

const headline = paragraphs.find((p) => p.startsWith('**'));
const caption = paragraphs.find((p) => p.startsWith('_') && p.endsWith('_'));
const intro = paragraphs.findIndex((p) => p.startsWith('gpui-styleguidist is'));
if (!headline || !caption || intro < 0) {
  throw new Error('README.md lost its headline, GIF caption or intro paragraph');
}

const unwrap = (text, mark) => text.slice(mark.length, -mark.length).replace(/\s+/g, ' ');
writeFileSync(
  join(site, 'src/readme-hero.json'),
  JSON.stringify({ headline: unwrap(headline, '**'), caption: unwrap(caption, '_') }, null, 2) + '\n',
);

// Relative links point into the repository on GitHub.
const body = paragraphs
  .slice(intro)
  .join('\n\n')
  .replace(/\]\((?!https?:|#|mailto:)([^)]+)\)/g, (_, path) => {
    const kind = /\.[a-z]+$/i.test(path) ? 'blob' : 'tree';
    return `](${repo}/${kind}/main/${path})`;
  });
writeFileSync(join(site, 'src/pages/_readme.mdx'), body);

copyFileSync(join(root, 'docs/demo.gif'), join(site, 'static/img/demo.gif'));

import Layout from '@theme/Layout';
import Link from '@docusaurus/Link';
import MDXContent from '@theme/MDXContent';
import TOC from '@theme/TOC';
import useBaseUrl from '@docusaurus/useBaseUrl';
import Readme, { toc } from './_readme.mdx';
import hero from '../readme-hero.json';
import styles from './index.module.css';

const facts = [
  { value: '1', label: 'closure per example' },
  { value: '5', label: 'outputs: web, native app, docs.rs, CI, llms.txt' },
  { value: '0', label: 'docs to keep in sync by hand' },
];

// The README's headline is one bold paragraph: its first sentence is the
// promise, the rest says what you get.
const [headline, subhead] = hero.headline.split(/(?<=\.) /, 2);

export default function Home() {
  return (
    <Layout description={hero.headline}>
      <header className={styles.hero}>
        <div className="container">
          <p className={styles.eyebrow}>React Styleguidist for GPUI</p>
          <h1 className={styles.headline}>{headline}</h1>
          <p className={styles.subhead}>{subhead}</p>
          <div className={styles.actions}>
            <Link className="button button--primary button--lg" href="pathname:///demo/" target="_self">
              Try the live demo
            </Link>
            <Link className="button button--secondary button--lg" href="#get-started">
              Get started
            </Link>
            <code className={styles.install}>cargo install cargo-styleguide</code>
          </div>
          <figure className={styles.figure}>
            <img src={useBaseUrl('/img/demo.gif')} width={1280} height={800} alt="The web styleguide: knobs, View code, the states grid and a dark preview" />
            <figcaption>{hero.caption.replace(/\s*\[Try it live\]\([^)]*\)\.?/, '')}</figcaption>
          </figure>
          <dl className={styles.facts}>
            {facts.map((fact) => (
              <div key={fact.label}>
                <dt>{fact.value}</dt>
                <dd>{fact.label}</dd>
              </div>
            ))}
          </dl>
        </div>
      </header>
      <main className="container margin-vert--xl">
        <div className="row">
          <article className={`col col--9 markdown ${styles.readme}`}>
            <MDXContent>
              <Readme />
            </MDXContent>
          </article>
          <aside className="col col--3">
            <TOC toc={toc} />
          </aside>
        </div>
      </main>
    </Layout>
  );
}

// Imported Markdown partials also export their table of contents.
declare module '*.mdx' {
  import type { ComponentType } from 'react';
  import type { TOCItem } from '@docusaurus/mdx-loader';

  export const toc: readonly TOCItem[];
  const MDXComponent: ComponentType;
  export default MDXComponent;
}

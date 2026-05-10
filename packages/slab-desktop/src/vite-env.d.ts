/// <reference types="vite/client" />

declare module 'markdown-it-katex' {
  import type MarkdownIt from 'markdown-it';

  const markdownItKatex: MarkdownIt.PluginSimple;
  export default markdownItKatex;
}

declare module 'markdown-it-emoji' {
  import type MarkdownIt from 'markdown-it';

  export const full: MarkdownIt.PluginSimple;
  export const light: MarkdownIt.PluginSimple;
  export const bare: MarkdownIt.PluginSimple;
}

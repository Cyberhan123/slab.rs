import MarkdownIt from "markdown-it"
import markdownItKatex from "markdown-it-katex"
import { full as markdownItEmoji } from "markdown-it-emoji"

const markdownRenderer = new MarkdownIt({
  html: false,
  linkify: true,
  typographer: true,
})
  .use(markdownItKatex)
  .use(markdownItEmoji)

export function renderWorkspaceMarkdown(content: string) {
  return markdownRenderer.render(content)
}

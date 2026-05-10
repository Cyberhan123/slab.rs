import "katex/dist/katex.min.css"

import { useMemo } from "react"

import { cn } from "@/lib/utils"
import { renderWorkspaceMarkdown } from "../lib/markdown-renderer"

type WorkspaceMarkdownPreviewProps = {
  className?: string
  content: string
}

export function WorkspaceMarkdownPreview({ className, content }: WorkspaceMarkdownPreviewProps) {
  const html = useMemo(() => renderWorkspaceMarkdown(content), [content])

  return (
    <div
      className={cn("workspace-markdown", className)}
      dangerouslySetInnerHTML={{ __html: html }}
    />
  )
}

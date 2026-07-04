"use client"

import * as React from "react"
import { code } from "@streamdown/code"
import { cjk } from "@streamdown/cjk"
import { mermaid } from "@streamdown/mermaid"
import { math } from '@streamdown/math';
import { Streamdown } from "streamdown"
import 'katex/dist/katex.min.css';

import { cn } from "@/lib/utils"

const DEFAULT_PLUGINS = { code, cjk, mermaid, math }

type MarkdownProps = React.ComponentProps<typeof Streamdown> & {
  hasNextChunk?: boolean
}

function Markdown({
  className,
  plugins = DEFAULT_PLUGINS,
  controls = false,
  hasNextChunk,
  ...props
}: MarkdownProps) {
  return (
    <Streamdown
      data-slot="markdown"
      plugins={plugins}
      controls={controls}
      mode={hasNextChunk ? "streaming" : props.mode}
      className={cn("cn-markdown w-full min-w-0 overflow-hidden", className)}
      {...props}
    />
  )
}

export { Markdown, Markdown as AssistantMarkdown }

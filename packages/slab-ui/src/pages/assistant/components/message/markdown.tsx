"use client"

import * as React from "react"
import { cjk } from "@streamdown/cjk"
import { mermaid } from "@streamdown/mermaid"
import { math } from '@streamdown/math';
import { Streamdown, useIsCodeFenceIncomplete } from "streamdown"
import 'streamdown/styles.css';
import 'katex/dist/katex.min.css';

import {
  CodeBlock,
  CodeBlockActions,
  CodeBlockContent,
  CodeBlockCopyButton,
  CodeBlockHeader,
  CodeBlockLine,
  CodeBlockTitle,
} from "@slab/components/code-block"
import { useHighlightedCode } from "@slab/components/hooks/use-highlighted-code"
import { cn } from "@slab/ui/lib/utils"

// The @streamdown/code plugin is dropped on purpose: fences are routed to the
// shadcn CodeBlock via the `components.code` override below, which does its
// own lazy highlighting (the plugin's shiki pipeline would fight the override).
const DEFAULT_PLUGINS = { cjk, mermaid, math }

type MarkdownProps = React.ComponentProps<typeof Streamdown> & {
  hasNextChunk?: boolean
}

type MarkdownCodeProps = React.ComponentProps<"code"> & {
  /** react-markdown hast node — consumed here so it never reaches the DOM. */
  node?: unknown
}

/**
 * The docs' CodeBlock↔Streamdown integration: fenced code blocks (the
 * `data-block` prop) render as a headered CodeBlock with per-line lazy
 * highlighting and a copy button that stays disabled while the fence is still
 * streaming in; inline code keeps the muted mono chip.
 */
function MarkdownCode({ node: _node, className, children, ...props }: MarkdownCodeProps) {
  const incomplete = useIsCodeFenceIncomplete()
  const block = "data-block" in props
  const code = typeof children === "string" ? children : ""
  const language = /language-(\S+)/.exec(className ?? "")?.[1] ?? ""
  const lines = useHighlightedCode(block ? code : "", language)

  if (!block) {
    return (
      <code className={cn("rounded-sm bg-muted px-1 py-0.5 font-mono text-[0.9em]", className)} {...props}>
        {children}
      </code>
    )
  }

  return (
    <CodeBlock className="my-3">
      <CodeBlockHeader>
        <CodeBlockTitle>{language || "text"}</CodeBlockTitle>
        <CodeBlockActions>
          <CodeBlockCopyButton value={code} disabled={incomplete} />
        </CodeBlockActions>
      </CodeBlockHeader>
      <CodeBlockContent>
        {lines.map((line, index) => {
          // Highlighted code lines are positional rows; identical content
          // (blank lines, braces) legitimately repeats, so there is no
          // data-dependent key to use instead of the row position.
          // oxlint-disable-next-line react/no-array-index-key -- positional rows; content repeats
          return <CodeBlockLine key={index}>{line}</CodeBlockLine>
        })}
      </CodeBlockContent>
    </CodeBlock>
  )
}

const markdownComponents = { code: MarkdownCode }

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
      // Drives useIsCodeFenceIncomplete inside the components override.
      isAnimating={hasNextChunk}
      components={markdownComponents}
      className={cn("cn-markdown w-full min-w-0 overflow-hidden", className)}
      {...props}
    />
  )
}

export { Markdown, Markdown as AssistantMarkdown }

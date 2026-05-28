import type { ComponentProps } from "@ant-design/x-markdown"
import XMarkdown from "@ant-design/x-markdown"
import Latex from "@ant-design/x-markdown/plugins/latex"
import "@ant-design/x-markdown/dist/plugins/latex.css"
import { CodeHighlighter } from "@ant-design/x"
import { isValidElement, memo, type ReactNode } from "react"

import { cn } from "@/lib/utils"

type AssistantMarkdownProps = {
  children: string
  className?: string
  hasNextChunk?: boolean
}

function childrenToText(children: ReactNode): string {
  if (typeof children === "string" || typeof children === "number") {
    return String(children)
  }

  if (Array.isArray(children)) {
    return children.map(childrenToText).join("")
  }

  if (isValidElement<{ children?: ReactNode }>(children)) {
    return childrenToText(children.props.children)
  }

  return ""
}

function SupComponent({ children, ...props }: ComponentProps) {
  return (
    <sup
      {...props}
      className={cn(
        "mx-0.5 inline-flex min-w-4 items-center justify-center rounded-full border border-current/20 px-1 text-[0.7em] font-semibold leading-4 text-[var(--brand-teal)]",
        props.className
      )}
    >
      {children}
    </sup>
  )
}

function CodeBlockComponent({ block, children, lang, ...props }: ComponentProps) {
  const code = childrenToText(children)
  if (!block) {
    return (
      <code {...props} className={props.className}>
        {children}
      </code>
    )
  }

  const language = lang?.trim().split(/\s+/)[0] || "text"

  return (
    <CodeHighlighter
      lang={language}
      className="my-3 overflow-hidden rounded-[14px] border border-border/60 text-xs"
      header={language}
    >
      {code}
    </CodeHighlighter>
  )
}

const markdownComponents = {
  code: CodeBlockComponent,
  sup: SupComponent,
}

const markdownConfig = {
  extensions: Latex(),
}

const streamingIdle = {
  enableAnimation: true,
  hasNextChunk: false,
}

const streamingActive = {
  enableAnimation: true,
  hasNextChunk: true,
}

function AssistantMarkdownView({
  children,
  className,
  hasNextChunk = false,
}: AssistantMarkdownProps) {
  return (
    <XMarkdown
      components={markdownComponents}
      config={markdownConfig}
      content={children}
      paragraphTag="div"
      className={cn("assistant-markdown text-base leading-[1.625]", className)}
      streaming={hasNextChunk ? streamingActive : streamingIdle}
    />
  )
}

export const AssistantMarkdown = memo(AssistantMarkdownView)

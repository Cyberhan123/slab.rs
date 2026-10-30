"use client"

/**
 * `read_file` tool part — registered under
 * `messagePartComponents.tools["read_file"]` so it takes over the generic tool
 * row whenever the part's toolName matches. Renders a file view (meta line +
 * headered CodeBlock with line numbers and lazy syntax highlighting) instead
 * of the raw `{"content":…}` envelope pretty-printed as JSON; without a
 * parseable envelope (call in flight, envelope drift) the body yields `null`
 * and `ToolPartRow` falls back to the generic Parameters/Result cards.
 */

import {
  CodeBlock,
  CodeBlockContent,
  CodeBlockHeader,
  CodeBlockLine,
  CodeBlockTitle,
} from "@slab/components/code-block"
import { useHighlightedCode } from "@slab/components/hooks/use-highlighted-code"
import type { ReactNode } from "react"

import { ToolPartRow } from "./message-tool-part"
import type { MessagePartRenderProps } from "./message-parts"
import type { TMessage, TMessagePart } from "./message-item"
import {
  DetailFootnote,
  DetailMeta,
  formatBytes,
  num,
  parseToolEnvelope,
  str,
} from "./tool-detail-shared"

/**
 * Extension → highlighter grammar for the `read_file` code view. Languages the
 * vendored highlighter does not ship (go, java) degrade to plain lines inside
 * the hook — the mapping documents the intent for when they are added.
 */
const EXTENSION_LANGUAGES: Record<string, string> = {
  rs: "rust",
  ts: "typescript",
  tsx: "tsx",
  mts: "typescript",
  cts: "typescript",
  js: "javascript",
  jsx: "jsx",
  mjs: "javascript",
  cjs: "javascript",
  json: "json",
  jsonc: "json",
  md: "markdown",
  mdx: "markdown",
  py: "python",
  pyi: "python",
  toml: "toml",
  yaml: "yaml",
  yml: "yaml",
  css: "css",
  scss: "css",
  html: "html",
  htm: "html",
  sql: "sql",
  go: "go",
  java: "java",
  sh: "bash",
  bash: "bash",
  zsh: "bash",
}

function languageForPath(path: string | undefined): string {
  const ext = path?.split(/[\\/]/).pop()?.split(".").pop()?.toLowerCase()
  return (ext && EXTENSION_LANGUAGES[ext]) || "plaintext"
}

/** The highlighted content lines (lazy: the grammar loads on first use). */
function ReadFileContent({
  content,
  language,
  startLine,
  numbered,
}: {
  content: string
  language: string
  startLine: number
  numbered: boolean
}) {
  const lines = useHighlightedCode(content, language)
  return (
    <CodeBlockContent lineNumbers={numbered} startLine={startLine}>
      {lines.map((line, index) => (
        <CodeBlockLine key={index}>{line}</CodeBlockLine>
      ))}
    </CodeBlockContent>
  )
}

function renderReadFileBody(input: unknown, output: unknown): ReactNode | null {
  const envelope = parseToolEnvelope(output)
  if (!envelope) return null
  const inputEnvelope = parseToolEnvelope(input)

  const path = str(inputEnvelope?.path)
  const content = str(envelope.content)
  if (content === undefined) return null
  const startLine = num(inputEnvelope?.start_line) ?? 1
  const totalLines = num(envelope.total_lines)
  const returnedLines = num(envelope.returned_lines)
  const totalBytes = num(envelope.total_bytes)
  const omittedBytes = num(envelope.omitted_bytes) ?? 0
  const endLine = returnedLines !== undefined ? startLine + returnedLines - 1 : totalLines
  return (
    <div className="flex flex-col gap-1.5">
      <DetailMeta>
        {path ? <span className="truncate">{path}</span> : null}
        {totalLines !== undefined ? (
          <span className="text-muted-foreground/70">
            L{startLine}
            {endLine !== undefined ? `–${endLine}` : ""} · {totalLines} lines
          </span>
        ) : null}
        {totalBytes !== undefined ? (
          <span className="text-muted-foreground/70">{formatBytes(totalBytes)}</span>
        ) : null}
      </DetailMeta>
      <CodeBlock>
        <CodeBlockHeader>
          <CodeBlockTitle>{path ?? "file"}</CodeBlockTitle>
        </CodeBlockHeader>
        <ReadFileContent
          content={content}
          language={languageForPath(path)}
          startLine={startLine}
          numbered={startLine === 1}
        />
      </CodeBlock>
      {omittedBytes > 0 ? (
        <DetailFootnote>truncated — {formatBytes(omittedBytes)} omitted</DetailFootnote>
      ) : null}
    </div>
  )
}

function MessageToolReadFilePart(props: MessagePartRenderProps<TMessagePart, TMessage>) {
  return <ToolPartRow {...props} renderBody={renderReadFileBody} />
}

export default MessageToolReadFilePart

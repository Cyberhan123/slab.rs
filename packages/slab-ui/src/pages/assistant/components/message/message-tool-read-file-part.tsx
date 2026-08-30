"use client"

/**
 * `read_file` tool part — registered under
 * `messagePartComponents.tools["read_file"]` so it takes over the generic tool
 * row whenever the part's toolName matches. Renders a file view (meta line +
 * syntax-highlighted content) instead of the raw `{"content":…}` envelope
 * pretty-printed as JSON; without a parseable envelope (call in flight,
 * envelope drift) the body yields `null` and `ToolPartRow` falls back to the
 * generic Parameters/Result cards.
 */

import type { ReactNode } from "react"
import type { BundledLanguage } from "shiki"

import { CodeBlock } from "./code-block"
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

/** Extension → shiki language for the `read_file` code view. */
const EXTENSION_LANGUAGES: Record<string, BundledLanguage> = {
  rs: "rust",
  ts: "typescript",
  tsx: "typescript",
  mts: "typescript",
  cts: "typescript",
  js: "javascript",
  jsx: "javascript",
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

function languageForPath(path: string | undefined): BundledLanguage {
  const ext = path?.split(/[\\/]/).pop()?.split(".").pop()?.toLowerCase()
  // `plaintext` is a shiki special language (valid at runtime) but absent from
  // the `BundledLanguage` union, hence the cast; CodeBlock falls back to plain
  // rendering for any language the highlighter hasn't loaded.
  return (ext && EXTENSION_LANGUAGES[ext]) || ("plaintext" as BundledLanguage)
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
    <div className="space-y-1.5">
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
      <CodeBlock code={content} language={languageForPath(path)} showLineNumbers={startLine === 1} />
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

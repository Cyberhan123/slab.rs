"use client"

/**
 * Per-tool structured detail bodies for the generic tool row's expanded view.
 *
 * Built-in read-side tools serialize their result as a JSON envelope
 * (`{"content":…}`, `{"entries":…}`, `{"matches":…}`); dumping that envelope as
 * a pretty-printed JSON card makes the user parse the tool's own wire format.
 * Each renderer here turns one known envelope into the natural UI for that
 * tool — a file view for `read_file`, an entry listing for `list_dir`, a path
 * list for `file_glob`, a match list for `grep` — and returns `null` for
 * anything unrecognized so `MessageToolPart` falls back to the generic
 * Parameters/Result cards (unknown tools, MCP calls, envelope drift).
 */

import type { BundledLanguage } from "shiki"
import { FileIcon, FileTextIcon, FolderIcon } from "lucide-react"
import type { ReactNode } from "react"

import { cn } from "@slab/ui/lib/utils"
import { CodeBlock } from "./code-block"

/** A tool result envelope, once unserialized. */
type Envelope = Record<string, unknown>

/**
 * Normalize a tool output into a parsed object. Tool results arrive either as
 * the raw JSON string the tool produced or as the already-parsed value; plain
 * strings (older outputs, plain-text results) are NOT envelopes.
 */
export function parseToolEnvelope(output: unknown): Envelope | null {
  if (typeof output === "string") {
    if (!output.startsWith("{")) return null
    try {
      return parseToolEnvelope(JSON.parse(output))
    } catch {
      return null
    }
  }
  if (typeof output === "object" && output !== null && !Array.isArray(output)) {
    return output as Envelope
  }
  return null
}

const str = (value: unknown): string | undefined =>
  typeof value === "string" && value.length > 0 ? value : undefined

const num = (value: unknown): number | undefined =>
  typeof value === "number" && Number.isFinite(value) ? value : undefined

/** Human-readable byte size (B / KB / MB / GB), input-safe for huge values. */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  const units = ["KB", "MB", "GB"]
  let value = bytes
  let unit = -1
  do {
    value /= 1024
    unit += 1
  } while (value >= 1024 && unit < units.length - 1)
  return `${value >= 10 ? Math.round(value) : value.toFixed(1)} ${units[unit]}`
}

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
  return (ext && EXTENSION_LANGUAGES[ext]) || "text"
}

/** Shared header/meta line above a detail body. */
const DetailMeta = ({ children }: { children: ReactNode }) => (
  <p className="flex flex-wrap items-center gap-x-2 gap-y-0.5 font-mono text-muted-foreground text-xs">
    {children}
  </p>
)

/** Shared footnote for truncated envelopes. */
const DetailFootnote = ({ children }: { children: ReactNode }) => (
  <p className="text-muted-foreground/80 text-xs italic">{children}</p>
)

// ── read_file ───────────────────────────────────────────────────────────────

/** `read_file` → a file view: meta line + syntax-highlighted content. */
function ReadFileBody({ input, output }: { input: Envelope | null; output: Envelope }) {
  const path = str(input?.path)
  const content = str(output.content)
  if (content === undefined) return null
  const startLine = num(input?.start_line) ?? 1
  const totalLines = num(output.total_lines)
  const returnedLines = num(output.returned_lines)
  const totalBytes = num(output.total_bytes)
  const omittedBytes = num(output.omitted_bytes) ?? 0
  const endLine =
    returnedLines !== undefined ? startLine + returnedLines - 1 : totalLines
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

// ── list_dir ────────────────────────────────────────────────────────────────

interface DirEntryLike {
  name?: unknown
  is_dir?: unknown
  size_bytes?: unknown
}

/** `list_dir` → the directory listing: icon + name + size per entry. */
function ListDirBody({ output }: { output: Envelope }) {
  const entries = Array.isArray(output.entries) ? (output.entries as DirEntryLike[]) : null
  if (!entries) return null
  if (entries.length === 0) {
    return <p className="text-muted-foreground text-xs italic">empty directory</p>
  }
  return (
    <div className="overflow-hidden rounded-md bg-muted/40" data-testid="tool-detail-dir">
      <ul className="max-h-64 divide-y divide-border/40 overflow-auto text-xs">
        {entries.map((entry) => {
          const name = str(entry.name) ?? "?"
          const isDir = entry.is_dir === true
          const size = num(entry.size_bytes)
          return (
            <li className="flex items-center gap-2 px-2 py-1" key={name}>
              {isDir ? (
                <FolderIcon className="size-3.5 shrink-0 text-sky-500" />
              ) : (
                <FileIcon className="size-3.5 shrink-0 text-muted-foreground" />
              )}
              <span className={cn("min-w-0 flex-1 truncate font-mono", isDir && "font-medium")}>
                {name}
              </span>
              {size !== undefined && !isDir ? (
                <span className="shrink-0 text-muted-foreground/70">{formatBytes(size)}</span>
              ) : null}
            </li>
          )
        })}
      </ul>
    </div>
  )
}

// ── file_glob ───────────────────────────────────────────────────────────────

interface PathMatchLike {
  path?: unknown
  kind?: unknown
}

/** `file_glob` → the matched-path list (file/folder icon + path). */
function GlobBody({ output }: { output: Envelope }) {
  const matches = Array.isArray(output.matches) ? (output.matches as PathMatchLike[]) : null
  if (!matches) return null
  const total = num(output.total) ?? matches.length
  const truncated = output.truncated === true
  if (matches.length === 0) {
    return <p className="text-muted-foreground text-xs italic">no matches</p>
  }
  return (
    <div className="space-y-1" data-testid="tool-detail-glob">
      <ul className="max-h-64 space-y-0.5 overflow-auto text-xs">
        {matches.map((match) => {
          const path = str(match.path) ?? "?"
          const isDir = match.kind === "dir"
          return (
            <li className="flex items-center gap-2" key={path}>
              {isDir ? (
                <FolderIcon className="size-3.5 shrink-0 text-sky-500" />
              ) : (
                <FileTextIcon className="size-3.5 shrink-0 text-muted-foreground" />
              )}
              <span className="truncate font-mono">{path}</span>
            </li>
          )
        })}
      </ul>
      <DetailFootnote>
        {total} match{total === 1 ? "" : "es"}
        {truncated ? " — list truncated" : ""}
      </DetailFootnote>
    </div>
  )
}

// ── grep ────────────────────────────────────────────────────────────────────

interface ContextLineLike {
  line?: unknown
  text?: unknown
}

interface GrepMatchLike {
  file?: unknown
  line?: unknown
  text?: unknown
  before_context?: unknown
  after_context?: unknown
}

const ContextLine = ({ entry }: { entry: ContextLineLike }) => (
  <div className="flex gap-2">
    <span className="w-8 shrink-0 text-right text-muted-foreground/50">
      {num(entry.line) ?? ""}
    </span>
    <span className="min-w-0 flex-1 break-all text-muted-foreground/70">
      {str(entry.text) ?? ""}
    </span>
  </div>
)

/** `grep` → a search-result list: `file:line` + context lines around the hit. */
function GrepBody({ output }: { output: Envelope }) {
  const matches = Array.isArray(output.matches) ? (output.matches as GrepMatchLike[]) : null
  if (!matches) return null
  const truncated = output.truncated === true
  const omitted = num(output.omitted_matches) ?? 0
  if (matches.length === 0) {
    return <p className="text-muted-foreground text-xs italic">no matches</p>
  }
  return (
    <div className="space-y-2" data-testid="tool-detail-grep">
      <ul className="max-h-80 space-y-2 overflow-auto">
        {matches.map((match) => {
          const file = str(match.file) ?? "?"
          const line = num(match.line)
          const before = Array.isArray(match.before_context)
            ? (match.before_context as ContextLineLike[])
            : []
          const after = Array.isArray(match.after_context)
            ? (match.after_context as ContextLineLike[])
            : []
          // One payload per matching line → `${file}:${line}` is unique.
          return (
            <li key={`${file}:${line}`}>
              <p className="font-mono text-muted-foreground text-xs">
                {file}
                {line !== undefined ? `:${line}` : ""}
              </p>
              <div className="mt-0.5 rounded-md bg-muted/40 p-1.5 font-mono text-xs">
                {before.map((entry) => (
                  <ContextLine entry={entry} key={`b${num(entry.line)}`} />
                ))}
                <div className="flex gap-2">
                  <span className="w-8 shrink-0 text-right text-muted-foreground/50">
                    {line ?? ""}
                  </span>
                  <span className="min-w-0 flex-1 break-all text-foreground">
                    {str(match.text) ?? ""}
                  </span>
                </div>
                {after.map((entry) => (
                  <ContextLine entry={entry} key={`a${num(entry.line)}`} />
                ))}
              </div>
            </li>
          )
        })}
      </ul>
      <DetailFootnote>
        {matches.length} match{matches.length === 1 ? "" : "es"}
        {omitted > 0 ? ` — ${omitted} more omitted` : truncated ? " — list truncated" : ""}
      </DetailFootnote>
    </div>
  )
}

// ── registry ────────────────────────────────────────────────────────────────

/**
 * Render the structured detail body for a known built-in tool, or `null` to
 * keep the generic Parameters/Result cards. `output` may be absent (call in
 * flight) — every renderer degrades to `null` without its envelope fields.
 */
export function renderToolDetailBody(
  toolName: string,
  input: unknown,
  output: unknown,
): ReactNode | null {
  const envelope = parseToolEnvelope(output)
  const inputEnvelope = parseToolEnvelope(input)
  if (toolName === "read_file") return envelope ? <ReadFileBody input={inputEnvelope} output={envelope} /> : null
  if (toolName === "list_dir") return envelope ? <ListDirBody output={envelope} /> : null
  if (toolName === "file_glob") return envelope ? <GlobBody output={envelope} /> : null
  if (toolName === "grep") return envelope ? <GrepBody output={envelope} /> : null
  return null
}

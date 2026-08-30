"use client"

/**
 * `grep` tool part — registered under
 * `messagePartComponents.tools["grep"]` so it takes over the generic tool row
 * whenever the part's toolName matches. Renders a search-result list
 * (`file:line` + context lines around the hit) instead of the raw
 * `{"matches":…}` envelope pretty-printed as JSON; without a parseable
 * envelope (call in flight, envelope drift) the body yields `null` and
 * `ToolPartRow` falls back to the generic Parameters/Result cards.
 */

import type { ReactNode } from "react"

import { ToolPartRow } from "./message-tool-part"
import type { MessagePartRenderProps } from "./message-parts"
import type { TMessage, TMessagePart } from "./message-item"
import { DetailFootnote, num, parseToolEnvelope, str } from "./tool-detail-shared"

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

function renderGrepBody(_input: unknown, output: unknown): ReactNode | null {
  const envelope = parseToolEnvelope(output)
  if (!envelope) return null
  const matches = Array.isArray(envelope.matches) ? (envelope.matches as GrepMatchLike[]) : null
  if (!matches) return null
  const truncated = envelope.truncated === true
  const omitted = num(envelope.omitted_matches) ?? 0
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

function MessageToolGrepPart(props: MessagePartRenderProps<TMessagePart, TMessage>) {
  return <ToolPartRow {...props} renderBody={renderGrepBody} />
}

export default MessageToolGrepPart

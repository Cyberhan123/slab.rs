"use client"

/**
 * `file_glob` tool part — registered under
 * `messagePartComponents.tools["file_glob"]` so it takes over the generic tool
 * row whenever the part's toolName matches. Renders the matched-path list
 * (file/folder icon + path + count footnote) instead of the raw
 * `{"matches":…}` envelope pretty-printed as JSON; without a parseable
 * envelope (call in flight, envelope drift) the body yields `null` and
 * `ToolPartRow` falls back to the generic Parameters/Result cards.
 */

import type { ReactNode } from "react"
import { FileTextIcon, FolderIcon } from "lucide-react"

import { ToolPartRow } from "./message-tool-part"
import type { MessagePartRenderProps } from "./message-parts"
import type { TMessage, TMessagePart } from "./message-item"
import { DetailFootnote, num, parseToolEnvelope, str } from "./tool-detail-shared"

interface PathMatchLike {
  path?: unknown
  kind?: unknown
}

function renderFileGlobBody(_input: unknown, output: unknown): ReactNode | null {
  const envelope = parseToolEnvelope(output)
  if (!envelope) return null
  const matches = Array.isArray(envelope.matches) ? (envelope.matches as PathMatchLike[]) : null
  if (!matches) return null
  const total = num(envelope.total) ?? matches.length
  const truncated = envelope.truncated === true
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

function MessageToolFileGlobPart(props: MessagePartRenderProps<TMessagePart, TMessage>) {
  return <ToolPartRow {...props} renderBody={renderFileGlobBody} />
}

export default MessageToolFileGlobPart

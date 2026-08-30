"use client"

/**
 * `list_dir` tool part — registered under
 * `messagePartComponents.tools["list_dir"]` so it takes over the generic tool
 * row whenever the part's toolName matches. Renders the directory listing
 * (icon + name + size per entry) instead of the raw `{"entries":…}` envelope
 * pretty-printed as JSON; without a parseable envelope (call in flight,
 * envelope drift) the body yields `null` and `ToolPartRow` falls back to the
 * generic Parameters/Result cards.
 */

import type { ReactNode } from "react"
import { FileIcon, FolderIcon } from "lucide-react"

import { cn } from "@slab/ui/lib/utils"

import { ToolPartRow } from "./message-tool-part"
import type { MessagePartRenderProps } from "./message-parts"
import type { TMessage, TMessagePart } from "./message-item"
import { formatBytes, num, parseToolEnvelope, str } from "./tool-detail-shared"

interface DirEntryLike {
  name?: unknown
  is_dir?: unknown
  size_bytes?: unknown
}

function renderListDirBody(_input: unknown, output: unknown): ReactNode | null {
  const envelope = parseToolEnvelope(output)
  if (!envelope) return null
  const entries = Array.isArray(envelope.entries) ? (envelope.entries as DirEntryLike[]) : null
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

function MessageToolListDirPart(props: MessagePartRenderProps<TMessagePart, TMessage>) {
  return <ToolPartRow {...props} renderBody={renderListDirBody} />
}

export default MessageToolListDirPart

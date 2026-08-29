"use client"

import { useMessageInteraction } from "../message-interaction-context"
import type { MessagePartRenderProps } from "./message-parts"
import type { TMessage, TMessagePart } from "./message-item"
import {
  isApprovalPending,
  deriveState,
  isToolActive,
  Tool,
  ToolContent,
  ToolHeader,
  type ToolPartLike,
} from "./message-tool-part"
import { PatchDiffView } from "../patch-diff-view"

/** A single file-change entry from a finalized `fileChange` item. */
interface FileChangeEntry {
  path?: string
  type?: string
  diff?: string
}

/** Parsed live progress line emitted by the apply_patch tool. */
interface PatchProgressLine {
  path?: string
  kind?: string
}

/**
 * Renders a `fileChange` (apply_patch) tool call as a readable diff card instead
 * of the generic JSON parameter/result cards used by `MessageToolPart`. The
 * intended diff is always shown; while the patch is still applying, the live
 * committed-file list (from `item/fileChange/outputDelta`) is shown above it so
 * the user can watch files apply as the patch runs.
 *
 * Registered under `messagePartComponents.tools["fileChange"]` so the parts
 * engine routes file-change tools here ahead of the generic `tool` renderer
 * (see `getMessagePartComponent`: `tools[name]` is checked before `tool`).
 */
function MessageToolFileChangePart({
  part,
  kind,
  toolCallId,
}: MessagePartRenderProps<TMessagePart, TMessage>) {
  if (kind !== "tool") return null

  const p = part as ToolPartLike
  const { approvalStatusByItemId, livePatchByItemId } = useMessageInteraction()
  const approval = toolCallId ? approvalStatusByItemId.get(toolCallId) : undefined
  const state = deriveState(p, approval)
  const active = isToolActive(state)

  const input = (p.input ?? {}) as { changes?: FileChangeEntry[] }
  const changes = input.changes ?? []
  const liveLines = toolCallId ? livePatchByItemId.get(toolCallId) : undefined

  return (
    <Tool defaultOpen={isApprovalPending(state)}>
      <ToolHeader title="apply_patch" state={state} />
      <ToolContent>
        {active && liveLines && liveLines.length > 0 ? (
          <ul className="space-y-1">
            {liveLines.map((line) => {
              let parsed: PatchProgressLine = {}
              try {
                parsed = JSON.parse(line) as PatchProgressLine
              } catch {
                parsed = { path: line }
              }
              return (
                <li
                  key={line}
                  className="flex items-center gap-2 rounded-md bg-muted/60 p-2 text-xs"
                >
                  <span className="font-mono text-muted-foreground">
                    {progressLabel(parsed.kind)}
                  </span>
                  <code className="font-mono">{parsed.path ?? "(file)"}</code>
                </li>
              )
            })}
          </ul>
        ) : null}
        <ul className="space-y-2" data-testid="assistant-tool-file-change">
          {changes.map((change) => (
            <li
              key={`${change.type}:${change.path}`}
              className="rounded-md bg-muted/60 p-2 text-xs"
            >
              <div className="flex items-center gap-2">
                <span className="font-mono text-muted-foreground">{change.type ?? "edit"}</span>
                <code className="font-mono">{change.path ?? "(file)"}</code>
              </div>
              {change.diff ? <PatchDiffView diff={change.diff} /> : null}
            </li>
          ))}
        </ul>
      </ToolContent>
    </Tool>
  )
}

/** Git-style one-letter label for a live apply_patch progress kind. */
function progressLabel(kind: string | undefined): string {
  switch (kind) {
    case "add":
      return "A"
    case "delete":
      return "D"
    default:
      return "M"
  }
}

export default MessageToolFileChangePart

"use client"

import { useMessageInteraction } from "../message-interaction-context"
import type { MessagePartRenderProps } from "./message-parts"
import type { TMessage, TMessagePart } from "./message-item"
import {
  Terminal,
  TerminalActions,
  TerminalContent,
  TerminalCopyButton,
  TerminalHeader,
  TerminalStatus,
  TerminalTitle,
} from "./terminal"
import {
  deriveState,
  isToolActive,
  Tool,
  ToolContent,
  ToolHeader,
  type ToolPartLike,
} from "./message-tool-part"

/**
 * Renders a `commandExecution` tool call as an interactive terminal (ANSI
 * output + streaming cursor) instead of the JSON parameter/result cards used by
 * generic tools. It reuses the shared tool-card chrome (`Tool`/`ToolHeader`/
 * `ToolContent`) from {@link message-tool-part} and swaps the body for a
 * composed `<Terminal>`:
 *   - `TerminalHeader` shows the agent's input (the command), via `TerminalTitle`.
 *   - `TerminalContent` shows only the command output (live while running, the
 *     finalized aggregated output once complete).
 *
 * Registered under `messagePartComponents.tools["commandExecution"]` so the
 * parts engine routes command tools here ahead of the generic `tool` renderer
 * (see `getMessagePartComponent`: `tools[name]` is checked before `tool`).
 */
function MessageToolCommandPart({
  part,
  kind,
  toolCallId,
}: MessagePartRenderProps<TMessagePart, TMessage>) {
  if (kind !== "tool") return null

  const p = part as ToolPartLike
  const { approvalStatusByItemId, liveOutputByItemId } = useMessageInteraction()
  const approval = toolCallId ? approvalStatusByItemId.get(toolCallId) : undefined
  const state = deriveState(p, approval)
  const active = isToolActive(state)

  const input = (p.input ?? {}) as { command?: string; cwd?: string }
  const command = input.command?.trim() ?? ""
  const cwd = input.cwd?.trim() ?? ""
  const finalizedOutput = typeof p.output === "string" ? p.output : p.errorText ?? ""
  // While the command is still running, render the streamed output deltas; once
  // it completes, the finalized output (aggregated by the server) takes over.
  const liveOutput = toolCallId ? liveOutputByItemId.get(toolCallId) : undefined
  const body = active && liveOutput !== undefined ? liveOutput : finalizedOutput

  return (
    <Tool defaultOpen={active}>
      <ToolHeader title="commandExecution" state={state} />
      <ToolContent>
        {/* `output` feeds both the TerminalContent body and the copy button, so
            it carries only the command output — the input lives in the header. */}
        <Terminal output={body} isStreaming={active}>
          <TerminalHeader>
            <TerminalTitle title={cwd || undefined}>
              {command ? `$ ${command}` : "Terminal"}
            </TerminalTitle>
            <div className="flex items-center gap-1">
              <TerminalStatus />
              <TerminalActions>
                <TerminalCopyButton />
              </TerminalActions>
            </div>
          </TerminalHeader>
          <TerminalContent />
        </Terminal>
      </ToolContent>
    </Tool>
  )
}

export default MessageToolCommandPart

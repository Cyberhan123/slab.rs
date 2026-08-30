"use client"

import { useLiveToolOutput, useMessageInteraction } from "../message-interaction-context"
import { summarizeToolCall } from "../../lib/tool-summaries"
import { ToolRow, ToolRowContent, ToolRowTrigger, toolRowIcon } from "./message-tool-row"
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
  isApprovalPending,
  deriveState,
  isToolActive,
  type ToolPartLike,
} from "./message-tool-part"

/** Parsed shape of the shell tool's `SandboxedOutput` JSON result. */
interface CommandExecutionOutput {
  stdout?: string
  stderr?: string
  exit_code?: number
  timed_out?: boolean
}

/**
 * Parse a finalized command output that the shell tool serializes as a
 * `{"stdout","stderr","exit_code","timed_out"}` JSON string. Returns `null` for
 * plain-text output (older rollouts, other command-like tools), so the terminal
 * falls back to rendering the raw string.
 */
export function parseCommandExecutionOutput(raw: string): CommandExecutionOutput | null {
  if (!raw.startsWith("{")) return null
  try {
    const value = JSON.parse(raw) as unknown
    if (typeof value !== "object" || value === null || Array.isArray(value)) return null
    const candidate = value as Record<string, unknown>
    if (typeof candidate.stdout !== "string" && typeof candidate.stderr !== "string") return null
    return candidate as CommandExecutionOutput
  } catch {
    return null
  }
}

/**
 * Renders a `commandExecution` tool call as a compact `Bash: <command>` row
 * (thinking-style, collapsed by default) whose expanded body is an interactive
 * terminal (ANSI output + streaming cursor) instead of the JSON cards used by
 * generic tools:
 *   - The collapsed trigger shows the one-line command summary; the cwd rides
 *     along as the row tooltip.
 *   - `TerminalHeader` shows the agent's input (the command), via `TerminalTitle`.
 *   - `TerminalContent` shows only the command output (live while running, the
 *     finalized aggregated output once complete). A finalized
 *     `SandboxedOutput` JSON result is split into stdout (terminal body) and
 *     stderr (rendered in the SAME terminal, in the error color, after
 *     stdout), instead of dumping the raw JSON envelope or a separate stderr
 *     block.
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
  const { approvalStatusByItemId } = useMessageInteraction()
  const { liveOutputByItemId } = useLiveToolOutput()
  const approval = toolCallId ? approvalStatusByItemId.get(toolCallId) : undefined
  const state = deriveState(p, approval)
  const active = isToolActive(state)

  const input = (p.input ?? {}) as { command?: string; cwd?: string }
  const command = input.command?.trim() ?? ""
  const cwd = input.cwd?.trim() ?? ""
  const summary = summarizeToolCall("commandExecution", input)
  const finalizedRaw = typeof p.output === "string" ? p.output : p.errorText ?? ""
  const parsed = finalizedRaw ? parseCommandExecutionOutput(finalizedRaw) : null
  const finalizedStdout = parsed ? parsed.stdout ?? "" : finalizedRaw
  const finalizedStderr = parsed?.stderr ?? ""
  // While the command is still running, render the streamed output deltas; once
  // it completes, the finalized output (aggregated by the server) takes over.
  const liveOutput = toolCallId ? liveOutputByItemId.get(toolCallId) : undefined
  const body = active && liveOutput !== undefined ? liveOutput : finalizedStdout

  return (
    <ToolRow defaultOpen={isApprovalPending(state)}>
      <ToolRowTrigger
        icon={toolRowIcon("commandExecution")}
        label={summary.label}
        detail={command || summary.detail}
        state={state}
        title={cwd || undefined}
      />
      <ToolRowContent>
        {/* `output` feeds both the TerminalContent body and the copy button, so
            it carries only the command output — the input lives in the trigger.
            Stderr renders INSIDE the terminal (error color, after stdout): a
            failed command's diagnostics belong in the console it ran in, not a
            separate block. */}
        <Terminal
          output={body}
          stderrText={active ? undefined : finalizedStderr || undefined}
          isStreaming={active}
        >
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
      </ToolRowContent>
    </ToolRow>
  )
}

export default MessageToolCommandPart

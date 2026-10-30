import { createChat, type AiSdkChat } from "@shadcn/helpers/ai-sdk"
import type { UIMessage } from "ai"

/**
 * Scripted assistant conversations for vitest (browser-mode) tests, built on
 * `@shadcn/helpers/ai-sdk`: deterministic multi-turn chats that stream through
 * the REAL `useChat` lifecycle via `chat.transport({ delayMs })` — no model,
 * no network, no Rust sidecar.
 *
 * Boundary: the helpers produce **message-part data only**. Slab's out-of-band
 * channels (ConversationState approvals map, `liveOutputByItemId`,
 * `livePatchByItemId`, subagent task maps) are still the test's job — provide
 * them through the usual `useHarnessConversation` / context mocks. This module
 * never replaces `HarnessChatTransport` in production wiring.
 *
 * Usage:
 *   const { chat, initialMessages } = scriptedReasoningThenCommand()
 *   render(<AssistantChatPane initialMessages={initialMessages()}
 *                             transport={chat.transport({ delayMs: 5 })} />)
 * Tests that only need a static transcript use `initialMessages` (= `chat.get(1)`)
 * with the existing echo-transport mocks; lifecycle tests drive `chat.transport`.
 */

/** Tool input/output shapes mirroring the harness wire envelopes. */
type ScriptedTools = {
  commandExecution: {
    input: { command?: string; cwd?: string }
    output: string
  }
  fileChange: {
    input: { changes?: Array<{ type: string; path: string; diff?: string }> }
    output: string
  }
  read_file: {
    input: { path?: string; start_line?: number }
    output: string
  }
  grep: {
    input: { pattern?: string }
    output: string
  }
  list_dir: {
    input: { path?: string }
    output: string
  }
  plan: {
    input: { goal?: string; steps?: Array<{ title: string; status: string }> }
    output: string
  }
  questionnaire: {
    input: { question?: string }
    output: string
  }
  delegate_subagent: {
    input: { task?: string }
    output: string
  }
  /** Not in `messagePartComponents.tools` — exercises the generic fallback row. */
  mcp_slab_stats: {
    input: { metric?: string }
    output: string
  }
}

/** The UIMessage variant the scripted chats produce (`tool-<name>` parts). */
type ScriptedMessage = UIMessage<unknown, never, ScriptedTools>
type ScriptedChatInstance = AiSdkChat<ScriptedMessage>

/** One scripted conversation: the chat plus lazily-materialized transcripts. */
export type ScriptedChat = {
  chat: ScriptedChatInstance
  /** The full transcript (`chat.get()`) — for static renders. */
  transcript: () => ScriptedMessage[]
  /** Just the first message — for `useChat({ messages })` hydration. */
  initialMessages: () => ScriptedMessage[]
}

function scripted(build: (chat: ScriptedChatInstance) => ScriptedChatInstance): ScriptedChat {
  const chat = build(createChat<ScriptedMessage>())
  return {
    chat,
    transcript: () => chat.get(),
    initialMessages: () => chat.get(1),
  }
}

/** A plain question and a plain answer — the minimal live cycle. */
export function scriptedTextOnly() {
  return scripted((chat) =>
    chat
      .user("What is slab?")
      .assistant(({ writer }) => {
        writer.text("slab is a local-first AI workspace.")
      })
  )
}

/** Reasoning, a command run and a follow-up answer in one turn. */
export function scriptedReasoningThenCommand() {
  return scripted((chat) =>
    chat
      .user("Run the test suite")
      .assistant(({ writer }) => {
        writer.reasoning("The user wants tests; I should run bun test in the workspace.")
        writer
          .tool("commandExecution", { input: { command: "bun run test", cwd: "/workspace" } })
          .output(JSON.stringify({ stdout: "922 passing\n", stderr: "", exit_code: 0 }))
        writer.text("All 922 tests pass.")
      })
  )
}

/** A command that fails; the row shows the error coloring. */
export function scriptedCommandError() {
  return scripted((chat) =>
    chat
      .user("Build the app")
      .assistant(({ writer }) => {
        writer
          .tool("commandExecution", { input: { command: "bun run build", cwd: "/workspace" } })
          .error("Command failed with exit code 124: timed out after 120s")
        writer.text("The build timed out.")
      })
  )
}

/** A file change with a patch diff per file. */
export function scriptedFileChange() {
  return scripted((chat) =>
    chat
      .user("Rename the helper")
      .assistant(({ writer }) => {
        writer
          .tool("fileChange", {
            input: {
              changes: [
                {
                  type: "edit",
                  path: "src/lib/utils.ts",
                  diff: "*** Begin Patch\n*** Update File: src/lib/utils.ts\n@@\n-export const cnOld = cn\n+export const cnNew = cn\n*** End Patch",
                },
              ],
            },
          })
          .output(JSON.stringify({ success: true }))
        writer.text("Renamed the helper.")
      })
  )
}

/** A finalized plan checklist card. */
export function scriptedPlan() {
  return scripted((chat) =>
    chat
      .user("Plan the migration")
      .assistant(({ writer }) => {
        writer
          .tool("plan", {
            input: {
              goal: "Migrate the message list",
              steps: [
                { title: "Vendor primitives", status: "completed" },
                { title: "Group runs", status: "in_progress" },
                { title: "Refresh baselines", status: "pending" },
              ],
            },
          })
          .output(JSON.stringify({ accepted: true }))
        writer.text("Here is the plan.")
      })
  )
}

/**
 * A plan tool paused on approval (`needsApproval`) — the transcript ends with
 * the pending tool part; the continuation turn materializes only after the
 * test resolves the approval through its own out-of-band mocks.
 */
export function scriptedPlanApprovalPending() {
  return scripted((chat) =>
    chat
      .user("Present the plan")
      .assistant(({ writer }) => {
        writer.tool("plan", {
          input: {
            goal: "Present for approval",
            steps: [{ title: "Approved step", status: "pending" }],
          },
          needsApproval: true,
        })
      })
  )
}

/** A read_file call whose body renders as a headered code view. */
export function scriptedReadFile() {
  return scripted((chat) =>
    chat
      .user("Show me the entrypoint")
      .assistant(({ writer }) => {
        writer
          .tool("read_file", { input: { path: "src/main.ts", start_line: 1 } })
          .output(
            JSON.stringify({
              content: 'import { createApp } from "./app"\n\ncreateApp()\n',
              total_lines: 3,
              returned_lines: 3,
              total_bytes: 58,
            })
          )
        writer.text("That is the entrypoint.")
      })
  )
}

/** A subagent delegation card. */
export function scriptedSubagentDelegation() {
  return scripted((chat) =>
    chat
      .user("Search the codebase")
      .assistant(({ writer }) => {
        writer
          .tool("delegate_subagent", { input: { task: "Find all uses of deriveState" } })
          .output(JSON.stringify({ status: "completed", summary: "3 matches" }))
        writer.text("Found 3 matches.")
      })
  )
}

/** An unknown tool — exercises the generic Parameters/Result fallback row. */
export function scriptedUnknownTool() {
  return scripted((chat) =>
    chat
      .user("Do the thing")
      .assistant(({ writer }) => {
        writer
          .tool("mcp_slab_stats", { input: { metric: "sessions" } })
          .output(JSON.stringify({ value: 42 }))
        writer.text("Done.")
      })
  )
}

/** Text → tools → text → tools → text: two Trace runs in one message. */
export function scriptedMultiRun() {
  return scripted((chat) =>
    chat
      .user("Investigate and fix")
      .assistant(({ writer }) => {
        writer.text("Looking into it.")
        writer.tool("grep", { input: { pattern: "deriveState" } }).output("message-tool-part.tsx:32")
        writer.text("Found the derivation.")
        writer
          .tool("read_file", { input: { path: "src/message-tool-part.tsx", start_line: 1 } })
          .output(JSON.stringify({ content: "export function deriveState()\n", total_lines: 1, returned_lines: 1 }))
        writer.text("Fixed.")
      })
  )
}

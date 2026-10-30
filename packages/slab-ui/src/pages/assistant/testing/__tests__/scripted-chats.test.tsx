import { useEffect, useMemo, useRef } from "react"
import { describe, expect, it, vi } from "vitest"
import { render } from "vitest-browser-react"
import { useChat } from "@ai-sdk/react"
import { MessageScrollerProvider } from "@slab/components/message-scroller"

import MessageList from "../../components/message-list/message-list"
import {
  scriptedCommandError,
  scriptedMultiRun,
  scriptedReasoningThenCommand,
  type ScriptedChat,
} from "../scripted-chats"

/**
 * Live-lifecycle coverage for the scripted-conversation fixtures: the helper
 * transport drives a REAL `useChat`, whose messages feed the real MessageList
 * pipeline (rows → part registry → Trace/ToolRow shells). This is the path
 * that previously needed the Rust sidecar or a fake WebSocket; the helpers
 * cover it in-process.
 */

function ScriptedThread({
  scripted,
  prompt,
  delayMs = 2,
}: {
  scripted: ScriptedChat
  prompt: string
  delayMs?: number
}) {
  const transport = useMemo(() => scripted.chat.transport({ delayMs }), [scripted, delayMs])
  const { messages, sendMessage, status } = useChat({ transport })
  const sent = useRef(false)
  useEffect(() => {
    if (!sent.current) {
      sent.current = true
      void sendMessage({ text: prompt })
    }
  }, [sendMessage, prompt])
  // The chat pane wraps the list in the provider (autoScroll follow); the
  // scroller hooks the streaming updates touch need it present.
  return (
    <MessageScrollerProvider autoScroll>
      <MessageList messages={messages} isBusy={status === "streaming"} />
    </MessageScrollerProvider>
  )
}

async function waitForToolState(state: string) {
  await vi.waitFor(
    () => {
      const line = document.querySelector(`span[data-tool-state="${state}"]`)
      expect(line).not.toBeNull()
    },
    { timeout: 10_000, interval: 50 }
  )
}

describe("scripted chats through the real useChat lifecycle", () => {
  it("streams reasoning, a command run and the final text into Trace rows", async () => {
    const screen = await render(
      <ScriptedThread scripted={scriptedReasoningThenCommand()} prompt="Run the test suite" />
    )

    // The command completed (finalized state on the line) and the closing
    // answer landed after it.
    await waitForToolState("output-available")
    await expect.element(screen.getByText("All 922 tests pass.")).toBeVisible()
    // The tool+reasoning rows share one Trace run.
    expect(document.querySelectorAll('[data-testid="assistant-trace-run"]').length).toBe(1)
  })

  it("keeps text and tool runs apart in the multi-run conversation", async () => {
    const screen = await render(
      <ScriptedThread scripted={scriptedMultiRun()} prompt="Investigate and fix" />
    )

    await expect.element(screen.getByText("Fixed.")).toBeVisible()
    await waitForToolState("output-available")
    // Two tool clusters separated by text parts → two Trace runs.
    expect(document.querySelectorAll('[data-testid="assistant-trace-run"]').length).toBe(2)
  })

  it("surfaces a failed command with the error state", async () => {
    const screen = await render(
      <ScriptedThread scripted={scriptedCommandError()} prompt="Build the app" />
    )

    await waitForToolState("output-error")
    await expect.element(screen.getByText("The build timed out.")).toBeVisible()
  })
})

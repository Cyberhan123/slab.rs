/**
 * React-glue tests for the thin `useHarnessConversation` hook. The conversation
 * state machine itself (restore, approvals, compaction, retries, actions) is
 * covered 1:1 by the core ConversationController node tests; what remains here
 * is the binding: controller-per-session lifecycle, useSyncExternalStore
 * snapshot stability, and transport rebuild on model change.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import { renderHook } from "vitest-browser-react"

import type { Thread } from "@slab/core/harness"
import { FakeWebSocket } from "@slab/core/harness/testing/fake-websocket"
import { useHarnessConversation } from "../use-harness-conversation"

/** Flush microtasks + the macrotask queue so the client's async open/await settle. */
function flush(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0))
}

function rpcResponse(id: number | string, result: unknown): string {
  return JSON.stringify({ jsonrpc: "2.0", id, result })
}

function rpcError(id: number | string, message: string): string {
  return JSON.stringify({ jsonrpc: "2.0", id, error: { code: -32000, message } })
}

const THREAD: Thread = {
  id: "hthread-1",
  preview: "",
  modelProvider: "",
  createdAt: 0,
  turns: [
    {
      id: "0",
      status: "completed",
      items: [
        { type: "userMessage", id: "u1", content: [{ type: "text", text: "hi" }] },
        { type: "agentMessage", id: "a1", text: "hello" },
      ],
    },
  ],
}

/** Drive open + the mandatory initialize handshake on the latest fake socket. */
async function driveOpenAndInit(socket = FakeWebSocket.last!): Promise<void> {
  await flush()
  socket.simOpen()
  await flush()
  const init = JSON.parse(socket.sent[0])
  socket.simMessage(rpcResponse(init.id, { protocolVersion: "1.0" }))
  await flush()
}

describe("useHarnessConversation", () => {
  beforeEach(() => {
    FakeWebSocket.reset("manual")
    vi.stubGlobal("WebSocket", FakeWebSocket)
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it("resets to a pristine controller when the session changes", async () => {
    const { result, rerender, unmount } = await renderHook(
      (props?: { sid: string | undefined }) => useHarnessConversation(props?.sid, "m1"),
      {
        initialProps: { sid: "s1" as string | undefined },
      },
    )
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(result.current.restoredThreadId).toBe("hthread-1"))
    expect(result.current.restoredMessages).toHaveLength(2)

    // Switch to a session-less render: a fresh controller means pristine state.
    await rerender({ sid: undefined })
    await vi.waitFor(() => expect(result.current.restoredThreadId).toBeNull())
    expect(result.current.restoredMessages).toHaveLength(0)
    expect(result.current.approvals).toHaveLength(0)
    expect(result.current.liveOutputByItemId.size).toBe(0)
    expect(result.current.activeConversation).toBeUndefined()
    await unmount()
  })

  it("keeps snapshot field references stable across unrelated re-renders", async () => {
    const { result, rerender, unmount } = await renderHook(
      (props?: { sid: string | undefined }) => useHarnessConversation(props?.sid, "m1"),
      {
        initialProps: { sid: "s1" as string | undefined },
      },
    )
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(result.current.restoredThreadId).toBe("hthread-1"))

    // Unrelated re-render (same props, new object identity) must not produce a
    // new restoredMessages reference — the external store snapshot is stable.
    const messages = result.current.restoredMessages
    await rerender({ sid: "s1" })
    expect(result.current.restoredMessages).toBe(messages)
    await unmount()
  })

  it("rebuilds the transport on a model change but keeps the conversation state", async () => {
    const { result, rerender, unmount } = await renderHook(
      (props?: { model: string }) => useHarnessConversation("s1", props?.model ?? "m1"),
      {
        initialProps: { model: "m1" },
      },
    )
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(result.current.restoredThreadId).toBe("hthread-1"))

    const transportBefore = result.current.transport
    await rerender({ model: "m2" })
    // New transport instance for the new model...
    expect(result.current.transport).not.toBe(transportBefore)
    // ...but the restored conversation survives (same controller).
    expect(result.current.restoredThreadId).toBe("hthread-1")
    expect(result.current.restoredMessages).toHaveLength(2)
    await unmount()
  })

  it("surfaces a restore error through the bound store", async () => {
    const { result, unmount } = await renderHook(() => useHarnessConversation("s3", "m1"))
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcError(req.id, "internal boom"))
    await flush()
    await vi.waitFor(() => {
      expect(result.current.error).toContain("internal boom")
    })
    await unmount()
  })
})

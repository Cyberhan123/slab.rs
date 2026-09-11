/**
 * React-glue tests for the thin `useHarnessConversation` hook. The conversation
 * state machine itself (restore, approvals, compaction, retries, actions) is
 * covered 1:1 by the core ConversationController node tests; what remains here
 * is the binding: controller-per-session lifecycle, useSyncExternalStore
 * snapshot stability, and transport rebuild on model change.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import { renderHook } from "vitest-browser-react"

import type { Thread } from "@slab/api/harness"
import { conversationPool } from "@slab/core/harness"
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
    // The hook binds controllers to the module-level pool; reset it so each
    // test mints its own controller under the freshly-stubbed WebSocket.
    conversationPool.disposeAll()
    FakeWebSocket.reset("manual")
    vi.stubGlobal("WebSocket", FakeWebSocket)
  })

  afterEach(() => {
    conversationPool.disposeAll()
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

  it("steers and interrupts through the detached action references the page destructures", async () => {
    const { result, unmount } = await renderHook(() => useHarnessConversation("s1", "m1"))
    await driveOpenAndInit()
    const resumeReq = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(resumeReq.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(result.current.restoredThreadId).toBe("hthread-1"))

    // The server owns the busy state: mark the thread running so steering is
    // the valid path (the queued chip renders from this).
    FakeWebSocket.last!.simMessage(
      JSON.stringify({
        jsonrpc: "2.0",
        method: "thread/statusChanged",
        params: { threadId: "hthread-1", status: "running" },
      }),
    )
    await flush()

    // Exactly what the assistant page does: destructure the action off the
    // hook's return value and call it later as a bare reference. A prototype
    // method would lose `this` and reject with a TypeError.
    const steer = result.current.sendSteering
    const pending = steer({
      id: "steer-1",
      role: "user",
      parts: [{ type: "text", text: "focus on the parser" }],
    })
    await flush()
    const turnReq = FakeWebSocket.last!.sent
      .map((raw) => JSON.parse(raw))
      .filter((m: { method?: string }) => m.method === "turn/start")
      .at(-1)!
    FakeWebSocket.last!.simMessage(
      rpcResponse(turnReq.id, { turn: { id: "0", status: "queued" }, queued: true }),
    )
    await expect(pending).resolves.toMatchObject({ queued: true })
    await vi.waitFor(() => expect(result.current.queuedTexts).toEqual(["focus on the parser"]))

    // The Stop control: the detached interrupt must reach the wire.
    const stop = result.current.interrupt
    const stopping = stop()
    await flush()
    const interruptReq = FakeWebSocket.last!.sent
      .map((raw) => JSON.parse(raw))
      .filter((m: { method?: string }) => m.method === "turn/interrupt")
      .at(-1)!
    expect(interruptReq.params).toMatchObject({ threadId: "hthread-1", turnId: "0" })
    FakeWebSocket.last!.simMessage(rpcResponse(interruptReq.id, {}))
    await expect(stopping).resolves.toBeUndefined()
    await unmount()
  })

  it("exposes the live-text mirror and the pane-detach action from the controller", async () => {
    const { result, unmount } = await renderHook(() => useHarnessConversation("s1", "m1"))
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(result.current.restoredThreadId).toBe("hthread-1"))

    // A live delta (not part of the restored history) lands in the mirror.
    FakeWebSocket.last!.simMessage(
      JSON.stringify({
        jsonrpc: "2.0",
        method: "item/agentMessage/delta",
        params: { threadId: "hthread-1", turnId: "1", itemId: "a9", delta: "live" },
      }),
    )
    await vi.waitFor(() =>
      expect(result.current.liveTextByItemId.get("a9")?.text).toBe("live"),
    )

    // Detach notification is wired through to the controller.
    expect(typeof result.current.notifyPaneDetachedMidRun).toBe("function")
    result.current.notifyPaneDetachedMidRun()
    // No resync fires while the thread is idle-terminal-less: the terminal
    // status hasn't arrived, so nothing observable yet — the controller tests
    // cover the terminal path. Just ensure the call does not throw.
    await flush()
    await unmount()
  })

  it("the transport is constructed with the controller lifecycle callbacks", async () => {
    const { result, unmount } = await renderHook(() => useHarnessConversation("s1", "m1"))
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(result.current.restoredThreadId).toBe("hthread-1"))

    // Drive the transport's send path against the fake socket: the
    // accepted-turn ack must advance the controller's turnStartSeq.
    expect(result.current.turnStartSeq).toBe(0)
    const stream = await result.current.transport.sendMessages({
      messages: [{ id: "u2", role: "user", parts: [{ type: "text", text: "go" }] }],
    })
    // Answer the turn/start the transport fired.
    const startReq = await vi.waitFor(() => {
      const req = FakeWebSocket.last!.sent
        .map((raw) => JSON.parse(raw))
        .filter((m: { method?: string }) => m.method === "turn/start")
        .at(-1)
      expect(req).toBeDefined()
      return req!
    })
    FakeWebSocket.last!.simMessage(rpcResponse(startReq.id, { turn: { id: "1", items: [], status: "inProgress" } }))
    FakeWebSocket.last!.simMessage(
      JSON.stringify({
        jsonrpc: "2.0",
        method: "turn/completed",
        params: { threadId: "hthread-1", turn: { id: "1", items: [], status: "completed" } },
      }),
    )
    await vi.waitFor(() => expect(result.current.turnStartSeq).toBe(1))
    // Drain the stream so the finally (onLocalStreamEnd) settles.
    const reader = stream.getReader()
    await reader.read()
    reader.releaseLock()
    await unmount()
  })
})

import { describe, expect, it, vi } from "vitest"
import type { UIMessage, UIMessageChunk } from "ai"

import { HarnessChatTransport } from "../harness-transport"
import type { HarnessClient } from "../harness-client"
import type { JsonRpcNotification, ThreadStartResult, TurnStartParams, TurnStartResult } from "../types"

interface FakeClientOptions {
  /** When set, `threadStart` resolves with this thread id (else `null` = fresh). */
  currentThreadId?: string | null
}

/** A minimal HarnessClient stand-in that synchronously replays a text turn. */
function makeFakeClient(options: FakeClientOptions = {}) {
  let handler: ((notification: JsonRpcNotification) => void) | null = null
  return {
    currentThreadId: options.currentThreadId ?? null,
    lastTurnIndex: -1,
    open: vi.fn(async () => {}),
    threadStart: vi.fn(async (): Promise<ThreadStartResult> => ({
      thread: { id: "hthread-1", preview: "", modelProvider: "", createdAt: 0, turns: [] },
      model: "slab-llama",
      modelProvider: "",
      cwd: "",
      approvalPolicy: "on-request",
      sandbox: { type: "workspaceWrite" },
    })),
    turnStart: vi.fn(async (params: TurnStartParams): Promise<TurnStartResult> => {
      const emit = (method: string, p: unknown) =>
        handler?.({ jsonrpc: "2.0", method, params: p } as JsonRpcNotification)
      emit("turn/started", { threadId: params.threadId, turn: { id: "0", items: [], status: "inProgress" } })
      emit("item/started", { item: { type: "agentMessage", id: "i1", text: "" }, threadId: params.threadId, turnId: "0" })
      emit("item/agentMessage/delta", { threadId: params.threadId, turnId: "0", itemId: "i1", delta: "hel" })
      emit("item/agentMessage/delta", { threadId: params.threadId, turnId: "0", itemId: "i1", delta: "lo" })
      emit("item/completed", { item: { type: "agentMessage", id: "i1", text: "hello" }, threadId: params.threadId, turnId: "0" })
      emit("turn/completed", { threadId: params.threadId, turn: { id: "0", items: [], status: "completed" } })
      return { turn: { id: "0", items: [], status: "inProgress" } }
    }),
    turnInterrupt: vi.fn(async () => ({ status: "interrupting" })),
    onNotification(h: (n: JsonRpcNotification) => void) {
      handler = h
      return () => {
        handler = null
      }
    },
  }
}

function userMessage(text: string): UIMessage {
  return { id: "u1", role: "user", parts: [{ type: "text", text }] }
}

async function collect(stream: ReadableStream<UIMessageChunk>): Promise<UIMessageChunk[]> {
  const reader = stream.getReader()
  const chunks: UIMessageChunk[] = []
  // eslint-disable-next-line no-constant-condition
  while (true) {
    const { value, done } = await reader.read()
    if (done) break
    if (value) chunks.push(value)
  }
  return chunks
}

describe("HarnessChatTransport", () => {
  it("starts a thread on a fresh session and streams a text turn", async () => {
    const fake = makeFakeClient({ currentThreadId: null })
    const transport = new HarnessChatTransport({
      client: fake as unknown as HarnessClient,
      model: "slab-llama",
    })

    const stream = await transport.sendMessages({ messages: [userMessage("hi")] })
    const chunks = await collect(stream)

    expect(fake.threadStart).toHaveBeenCalledOnce()
    expect(fake.turnStart).toHaveBeenCalledOnce()
    // The transport bound the thread id returned by threadStart.
    expect(fake.currentThreadId).toBe("hthread-1")
    expect(chunks.map((c) => c.type)).toEqual([
      "text-start",
      "text-delta",
      "text-delta",
      "text-end",
      "finish-step",
      "finish",
    ])
  })

  it("reuses the bound thread on subsequent turns", async () => {
    const fake = makeFakeClient({ currentThreadId: "hthread-9" })
    const transport = new HarnessChatTransport({
      client: fake as unknown as HarnessClient,
      model: "slab-llama",
    })

    await collect(await transport.sendMessages({ messages: [userMessage("again")] }))

    expect(fake.threadStart).not.toHaveBeenCalled()
    expect(fake.turnStart).toHaveBeenCalledWith(
      expect.objectContaining({ threadId: "hthread-9" }),
    )
  })

  it("returns null from reconnectToStream (no resumable stream)", async () => {
    const transport = new HarnessChatTransport({
      client: makeFakeClient() as unknown as HarnessClient,
    })
    await expect(transport.reconnectToStream()).resolves.toBeNull()
  })
})

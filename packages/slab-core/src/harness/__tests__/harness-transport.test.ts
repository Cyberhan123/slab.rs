import { describe, expect, it, vi } from "vitest"
import type { UIMessage, UIMessageChunk } from "ai"

import { HarnessChatTransport } from "../harness-transport"
import type { HarnessClient } from "../harness-client"
import type { JsonRpcNotification, ThreadStartResult, TurnStartParams, TurnStartResult } from "@slab/api/harness"

interface FakeClientOptions {
  /** When set, `threadStart` resolves with this thread id (else `null` = fresh). */
  currentThreadId?: string | null
}

/** A minimal HarnessClient stand-in that synchronously replays a text turn. */
function makeFakeClient(options: FakeClientOptions = {}) {
  let handler: ((notification: JsonRpcNotification) => void) | null = null
  const emit = (method: string, p: unknown) =>
    handler?.({ jsonrpc: "2.0", method, params: p } as JsonRpcNotification)
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
      emit("turn/started", { threadId: params.threadId, turn: { id: "0", items: [], status: "inProgress" } })
      emit("item/started", { item: { type: "agentMessage", id: "i1", text: "" }, threadId: params.threadId, turnId: "0" })
      emit("item/agentMessage/delta", { threadId: params.threadId, turnId: "0", itemId: "i1", delta: "hel" })
      emit("item/agentMessage/delta", { threadId: params.threadId, turnId: "0", itemId: "i1", delta: "lo" })
      emit("item/completed", { item: { type: "agentMessage", id: "i1", text: "hello" }, threadId: params.threadId, turnId: "0" })
      emit("turn/completed", { threadId: params.threadId, turn: { id: "0", items: [], status: "completed" } })
      return { turn: { id: "0", items: [], status: "inProgress" } }
    }),
    turnInterrupt: vi.fn(async () => ({ status: "interrupting" })),
    /** Emit a notification to the currently-registered handler (test seam). */
    emitNotification: emit,
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

function userMessageWithImage(text: string, url: string): UIMessage {
  return {
    id: "u1",
    role: "user",
    parts: [
      { type: "text", text },
      { type: "file", mediaType: "image/png", url },
    ],
  }
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

  it("maps an image file part to a harness image input (data-URL web form)", async () => {
    const fake = makeFakeClient({ currentThreadId: "hthread-9" })
    const transport = new HarnessChatTransport({
      client: fake as unknown as HarnessClient,
      model: "slab-llama",
    })

    await collect(
      await transport.sendMessages({
        messages: [userMessageWithImage("describe this", "data:image/png;base64,iVBOR=")],
      }),
    )

    expect(fake.turnStart).toHaveBeenCalledWith(
      expect.objectContaining({
        input: expect.arrayContaining([
          expect.objectContaining({ type: "text", text: "describe this" }),
          expect.objectContaining({ type: "image", imageUrl: "data:image/png;base64,iVBOR=" }),
        ]),
      }),
    )
  })

  it("returns null from reconnectToStream (no resumable stream)", async () => {
    const transport = new HarnessChatTransport({
      client: makeFakeClient() as unknown as HarnessClient,
    })
    await expect(transport.reconnectToStream()).resolves.toBeNull()
  })

  // ── open retry + local-stream lifecycle callbacks ─────────────────────────

  it("retries client.open with backoff before failing the stream", async () => {
    const fake = makeFakeClient({ currentThreadId: "hthread-1" })
    let failures = 0
    fake.open = vi.fn(async () => {
      failures += 1
      if (failures <= 2) throw new Error("socket not ready")
    })
    const transport = new HarnessChatTransport({
      client: fake as unknown as HarnessClient,
    })

    const stream = await transport.sendMessages({ messages: [userMessage("hi")] })
    await collect(stream)

    expect(fake.open).toHaveBeenCalledTimes(3)
    expect(fake.turnStart).toHaveBeenCalledOnce()
  })

  it("fires onLocalStreamBegin / onLocalTurnStarted / onLocalStreamEnd in order", async () => {
    const fake = makeFakeClient({ currentThreadId: "hthread-1" })
    const calls: string[] = []
    const transport = new HarnessChatTransport({
      client: fake as unknown as HarnessClient,
      onLocalStreamBegin: () => calls.push("begin"),
      onLocalTurnStarted: (threadId) => calls.push(`started:${threadId}`),
      onLocalStreamEnd: () => calls.push("end"),
    })

    const stream = await transport.sendMessages({ messages: [userMessage("hi")] })
    // Begin fires before the stream body runs (the first chunk may already
    // be readable before the turn completes).
    expect(calls[0]).toBe("begin")
    await collect(stream)

    expect(calls).toEqual(["begin", "started:hthread-1", "end"])
  })

  it("no turn-started ack when turn/start rejects", async () => {
    const fake = makeFakeClient({ currentThreadId: "hthread-1" })
    fake.turnStart = vi.fn(async () => {
      throw new Error("model not found")
    })
    const started = vi.fn()
    const transport = new HarnessChatTransport({
      client: fake as unknown as HarnessClient,
      onLocalTurnStarted: started,
      onLocalStreamEnd: () => {},
    })

    const stream = await transport.sendMessages({ messages: [userMessage("hi")] })
    const chunks = await collect(stream)

    expect(started).not.toHaveBeenCalled()
    expect(chunks.some((chunk) => chunk.type === "error")).toBe(true)
  })

  it("replayed completed items mid-stream do not duplicate text parts", async () => {
    // A reconnect under a live transport subscription re-delivers completed
    // items (the server's replay buffer); per-item open/close bookkeeping in
    // stream.ts must absorb them (exactly one text-start/text-end pair).
    const fake = makeFakeClient({ currentThreadId: "hthread-1" })
    fake.lastTurnIndex = 0
    fake.turnStart = vi.fn(async (params: TurnStartParams): Promise<TurnStartResult> => {
      fake.emitNotification("item/started", {
        item: { type: "agentMessage", id: "i1", text: "" },
        threadId: params.threadId,
        turnId: "5",
      })
      fake.emitNotification("item/completed", {
        item: { type: "agentMessage", id: "i1", text: "hello" },
        threadId: params.threadId,
        turnId: "5",
      })
      // The replayed duplicate: same item id, turnId above the threshold.
      fake.emitNotification("item/completed", {
        item: { type: "agentMessage", id: "i1", text: "hello" },
        threadId: params.threadId,
        turnId: "5",
      })
      fake.emitNotification("turn/completed", {
        threadId: params.threadId,
        turn: { id: "5", items: [], status: "completed" },
      })
      return { turn: { id: "5", items: [], status: "inProgress" } }
    })
    const transport = new HarnessChatTransport({
      client: fake as unknown as HarnessClient,
    })

    const chunks = await collect(await transport.sendMessages({ messages: [userMessage("hi")] }))
    expect(chunks.filter((chunk) => chunk.type === "text-start")).toHaveLength(1)
    expect(chunks.filter((chunk) => chunk.type === "text-end")).toHaveLength(1)
  })
})

import { beforeEach, describe, expect, it } from "vitest"

import { FakeWebSocket } from "./fake-websocket"
import { HarnessClient } from "../harness-client"
import { HARNESS_METHOD } from "../types"

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

function notification(method: string, params: unknown): string {
  return JSON.stringify({ jsonrpc: "2.0", method, params })
}

function makeClient(sessionId = "s1"): HarnessClient {
  return new HarnessClient({
    sessionId,
    WebSocketCtor: FakeWebSocket as unknown as typeof WebSocket,
  })
}

/** Drive the initialize handshake on the most recent fake socket. */
async function driveInitialize(): Promise<number | string> {
  await flush()
  const socket = FakeWebSocket.last!
  socket.simOpen()
  await flush()
  const initialize = JSON.parse(socket.sent[0])
  socket.simMessage(rpcResponse(initialize.id, { protocolVersion: "1.0" }))
  await flush()
  return initialize.id
}

describe("HarnessClient", () => {
  beforeEach(() => {
    FakeWebSocket.reset("manual")
  })

  it("connects with the session token on the WS url", async () => {
    const client = makeClient("abc")
    const ready = client.open()
    await flush()
    expect(FakeWebSocket.last?.url).toContain("/v1/agents/harness")
    expect(FakeWebSocket.last?.url).toContain("token=abc")

    FakeWebSocket.last!.simOpen()
    await flush()
    const initialize = JSON.parse(FakeWebSocket.last!.sent[0])
    FakeWebSocket.last!.simMessage(rpcResponse(initialize.id, { protocolVersion: "1.0" }))
    await ready
    expect(client.getStatus()).toBe("ready")
    client.close()
  })

  it("correlates a request with its response", async () => {
    const client = makeClient()
    const opened = client.open()
    await driveInitialize()
    await opened

    const result = client.threadStart({ model: "slab-llama" })
    await flush()
    const request = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    expect(request.method).toBe(HARNESS_METHOD.THREAD_START)
    FakeWebSocket.last!.simMessage(rpcResponse(request.id, { thread: { id: "hthread-1", preview: "", modelProvider: "", createdAt: 0, turns: [] }, model: "slab-llama", modelProvider: "", cwd: "", approvalPolicy: "on-request", sandbox: { type: "workspaceWrite" } }))
    await expect(result).resolves.toMatchObject({ thread: { id: "hthread-1" } })
    client.close()
  })

  it("rejects on an error response", async () => {
    const client = makeClient()
    const opened = client.open()
    await driveInitialize()
    await opened

    const result = client.threadResume({})
    await flush()
    const request = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcError(request.id, "no thread to resume for session"))
    await expect(result).rejects.toThrow("no thread to resume")
    client.close()
  })

  it("dispatches server notifications to subscribers", async () => {
    const client = makeClient()
    const opened = client.open()
    await driveInitialize()
    await opened

    const received: string[] = []
    client.onNotification((n) => received.push(n.method))
    FakeWebSocket.last!.simMessage(notification("item/agentMessage/delta", { threadId: "t", turnId: "0", itemId: "i", delta: "hi" }))
    await flush()
    expect(received).toEqual(["item/agentMessage/delta"])
    client.close()
  })

  it("transitions to closed and rejects pending requests on close", async () => {
    const client = makeClient()
    const opened = client.open()
    await driveInitialize()
    await opened

    const statuses: string[] = []
    client.onStatusChange((s) => statuses.push(s))
    const result = client.threadResume({})
    await flush()
    FakeWebSocket.last!.simClose()
    await expect(result).rejects.toThrow("harness socket closed")
    expect(client.getStatus()).toBe("closed")
    expect(statuses).toContain("closed")
  })
})

import { beforeEach, describe, expect, it, vi } from "vitest"

import { FakeWebSocket } from "../testing/fake-websocket"
import {
  ConversationController,
  MAX_RESTORE_ATTEMPTS,
  RESTORE_BACKOFF_MS,
} from "../conversation-controller"
import { HARNESS_NOTIFICATION, type Thread } from "../types"

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

function makeController(sessionId?: string): ConversationController {
  return new ConversationController({
    sessionId,
    WebSocketCtor: FakeWebSocket as unknown as typeof WebSocket,
  })
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

describe("ConversationController", () => {
  beforeEach(() => {
    FakeWebSocket.reset("manual")
  })

  // ── store mechanics ──────────────────────────────────────────────────────

  it("starts with a pristine snapshot and a reference-stable getState", () => {
    const controller = makeController("s1")
    const state = controller.getState()
    expect(state.restoredMessages).toEqual([])
    expect(state.restoredThreadId).toBeNull()
    expect(state.activeConversation).toBeUndefined()
    expect(state.restoreVersion).toBe(0)
    expect(state.isHistoryLoading).toBe(false)
    expect(state.error).toBeNull()
    expect(state.actionError).toBeNull()
    expect(state.approvals).toEqual([])
    expect(state.approvalStatusByItemId.size).toBe(0)
    expect(state.liveOutputByItemId.size).toBe(0)
    expect(state.livePatchByItemId.size).toBe(0)
    expect(state.modelLoad).toBeNull()
    expect(state.turnUsage).toBeNull()
    expect(state.historyCreatedAt).toBeNull()
    expect(state.commands).toEqual([])
    expect(state.compactionMarkers).toEqual([])
    expect(state.isCompacting).toBe(false)
    expect(state.isForking).toBe(false)
    expect(state.isRollingBack).toBe(false)
    expect(state.userMessageTurnIndex.size).toBe(0)
    expect(state.planMode).toBe(false)
    // No change between calls → the same snapshot reference.
    expect(controller.getState()).toBe(state)
  })

  it("notifies subscribers on changes and stops after unsubscribe", () => {
    const controller = makeController()
    const events: number[] = []
    const unsubscribe = controller.subscribe(() => events.push(events.length))

    controller.setPlanMode(true)
    expect(controller.getState().planMode).toBe(true)
    expect(events).toHaveLength(1)

    unsubscribe()
    controller.setPlanMode(false)
    expect(controller.getState().planMode).toBe(false)
    expect(events).toHaveLength(1)
  })

  // ── restore machine (ported 1:1 from the former hook tests) ──────────────

  it("restores a resumed thread into messages and binds the thread id", async () => {
    const controller = makeController("s1")
    controller.start()

    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()

    await vi.waitFor(() => {
      expect(controller.getState().isHistoryLoading).toBe(false)
    })
    expect(controller.getState().restoredThreadId).toBe("hthread-1")
    expect(controller.getState().activeConversation).toBe("s1")
    expect(controller.getState().restoredMessages).toHaveLength(2)
    expect(controller.getState().error).toBeNull()
  })

  it("treats a 'no thread to resume' rejection as a fresh session", async () => {
    const controller = makeController("s2")
    controller.start()

    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcError(req.id, "no thread to resume for session"))
    await flush()

    await vi.waitFor(() => {
      expect(controller.getState().isHistoryLoading).toBe(false)
    })
    expect(controller.getState().restoredThreadId).toBeNull()
    expect(controller.getState().restoredMessages).toHaveLength(0)
    expect(controller.getState().error).toBeNull()
  })

  it("surfaces an unexpected resume error", async () => {
    const controller = makeController("s3")
    controller.start()

    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcError(req.id, "internal boom"))
    await flush()

    await vi.waitFor(() => {
      expect(controller.getState().error).toContain("internal boom")
    })
    expect(controller.getState().isHistoryLoading).toBe(false)
  })

  it("tracks a command-execution approval request and ignores other threads", async () => {
    const controller = makeController("s1")
    controller.start()
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(controller.getState().restoredThreadId).toBe("hthread-1"))

    // Approval for the bound thread → tracked.
    FakeWebSocket.last!.simMessage(
      notification("item/commandExecution/requestApproval", {
        threadId: "hthread-1",
        turnId: "1",
        itemId: "call-1",
        command: "echo hi",
        cwd: "/tmp",
        reason: "shell",
        category: "shell",
        allowedScopes: ["run_once", "always_in_workspace"],
      }),
    )
    await flush()
    await vi.waitFor(() => expect(controller.getState().approvals).toHaveLength(1))
    expect(controller.getState().approvals[0]).toMatchObject({
      itemId: "call-1",
      command: "echo hi",
      status: "pending",
    })
    expect(controller.getState().approvalStatusByItemId.get("call-1")).toBe("pending")

    // Approval for a different thread → ignored.
    FakeWebSocket.last!.simMessage(
      notification("item/commandExecution/requestApproval", {
        threadId: "other-thread",
        turnId: "1",
        itemId: "call-2",
        command: "rm -rf /",
        cwd: "/",
      }),
    )
    await flush()
    expect(controller.getState().approvals).toHaveLength(1)
  })

  it("resolves an approval optimistically and keeps it approved when delivered", async () => {
    const controller = makeController("s1")
    controller.start()
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(controller.getState().restoredThreadId).toBe("hthread-1"))
    FakeWebSocket.last!.simMessage(
      notification("item/commandExecution/requestApproval", {
        threadId: "hthread-1",
        turnId: "1",
        itemId: "call-1",
        command: "echo hi",
        cwd: "/tmp",
      }),
    )
    await flush()
    await vi.waitFor(() => expect(controller.getState().approvals).toHaveLength(1))

    const p = controller.resolveApproval("call-1", true, "run_once")
    await flush()
    const resolveReq = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    expect(resolveReq.method).toBe("approval/resolve")
    FakeWebSocket.last!.simMessage(
      rpcResponse(resolveReq.id, { delivered: true, status: "approved" }),
    )
    await flush()
    await expect(p).resolves.toBeUndefined()
    // Approved ⇒ no longer in the pending list, but status map records it.
    await vi.waitFor(() => expect(controller.getState().approvals).toHaveLength(0))
    expect(controller.getState().approvalStatusByItemId.get("call-1")).toBe("approved")
  })

  it("reverts to pending and rejects when delivery failed (delivered=false)", async () => {
    const controller = makeController("s1")
    controller.start()
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(controller.getState().restoredThreadId).toBe("hthread-1"))
    FakeWebSocket.last!.simMessage(
      notification("item/commandExecution/requestApproval", {
        threadId: "hthread-1",
        turnId: "1",
        itemId: "call-1",
        command: "echo hi",
        cwd: "/tmp",
      }),
    )
    await flush()
    await vi.waitFor(() => expect(controller.getState().approvals).toHaveLength(1))

    // Capture the rejection (attached before the response arrives) so it is
    // never momentarily unhandled.
    const p = controller.resolveApproval("call-1", true, "run_once")
    const captured = p.then(
      () => new Error("expected resolveApproval to reject, but it resolved"),
      (err: unknown) => err,
    )
    await flush()
    const resolveReq = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(resolveReq.id, { delivered: false }))
    await flush()
    expect(String(await captured)).toContain("approval not delivered")
    await vi.waitFor(() =>
      expect(controller.getState().approvalStatusByItemId.get("call-1")).toBe("pending"),
    )
    expect(controller.getState().approvals).toHaveLength(1)
  })

  it("clears plan mode when a plan approval is approved", async () => {
    const controller = makeController("s1")
    controller.start()
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(controller.getState().restoredThreadId).toBe("hthread-1"))
    controller.setPlanMode(true)
    expect(controller.getState().planMode).toBe(true)
    FakeWebSocket.last!.simMessage(
      notification("item/commandExecution/requestApproval", {
        threadId: "hthread-1",
        turnId: "1",
        itemId: "plan-1",
        command: "present_plan",
        cwd: "/tmp",
        planSnapshot: {
          plan_id: "p1",
          summary: "s",
          items: [],
          counts: { pending: 0, in_progress: 0, completed: 0, blocked: 0 },
        },
      }),
    )
    await flush()
    await vi.waitFor(() => expect(controller.getState().approvals[0]?.kind).toBe("plan"))

    const p = controller.resolveApproval("plan-1", true, "run_once")
    await flush()
    const resolveReq = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(resolveReq.id, { delivered: true }))
    await p
    expect(controller.getState().planMode).toBe(false)
  })

  it("accumulates live command output under the 256 KiB per-item cap", async () => {
    const controller = makeController("s1")
    controller.start()
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(controller.getState().restoredThreadId).toBe("hthread-1"))

    FakeWebSocket.last!.simMessage(
      notification("item/commandExecution/outputDelta", {
        threadId: "hthread-1",
        turnId: "1",
        itemId: "call-1",
        delta: "part-1\n",
      }),
    )
    await flush()
    FakeWebSocket.last!.simMessage(
      notification("item/commandExecution/outputDelta", {
        threadId: "hthread-1",
        turnId: "1",
        itemId: "call-1",
        delta: "part-2\n",
      }),
    )
    await flush()
    await vi.waitFor(() =>
      expect(controller.getState().liveOutputByItemId.get("call-1")).toBe("part-1\npart-2\n"),
    )

    // A delta that would exceed the 256 KiB cap is dropped (existing output kept).
    const over = "x".repeat(256 * 1024 + 8)
    FakeWebSocket.last!.simMessage(
      notification("item/commandExecution/outputDelta", {
        threadId: "hthread-1",
        turnId: "1",
        itemId: "call-1",
        delta: over,
      }),
    )
    await flush()
    expect(controller.getState().liveOutputByItemId.get("call-1")).toBe("part-1\npart-2\n")

    // Output for a different thread is ignored.
    FakeWebSocket.last!.simMessage(
      notification("item/commandExecution/outputDelta", {
        threadId: "other",
        turnId: "1",
        itemId: "call-1",
        delta: "ignored",
      }),
    )
    await flush()
    expect(controller.getState().liveOutputByItemId.get("call-1")).toBe("part-1\npart-2\n")
  })

  it("forks the current thread and rebinds the socket to the child", async () => {
    const controller = makeController("s1")
    controller.start()
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(controller.getState().restoredThreadId).toBe("hthread-1"))

    const CHILD: Thread = { ...THREAD, id: "hthread-child", createdAt: 123 }

    const p = controller.forkThread()
    await flush()
    // thread/fork → returns the child thread (the controller only consumes child.id).
    const forkReq = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    expect(forkReq.method).toBe("thread/fork")
    FakeWebSocket.last!.simMessage(rpcResponse(forkReq.id, { thread: CHILD }))
    await flush()
    // thread/resume of the child → returns the child's copied history.
    const resumeReq = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    expect(resumeReq.method).toBe("thread/resume")
    expect(resumeReq.params).toMatchObject({ threadId: "hthread-child" })
    FakeWebSocket.last!.simMessage(rpcResponse(resumeReq.id, { thread: CHILD }))
    await flush()
    await p

    await vi.waitFor(() => expect(controller.getState().restoredThreadId).toBe("hthread-child"))
    expect(controller.getState().isForking).toBe(false)
    expect(controller.getState().error).toBeNull()
    expect(controller.getState().restoredMessages).toHaveLength(2)
  })

  it("fetches the command registry on restore and exposes it as commands", async () => {
    const controller = makeController("s1")
    controller.start()
    await driveOpenAndInit()

    // Complete thread/resume so the restore path settles.
    const resumeReq = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    expect(resumeReq.method).toBe("thread/resume")
    FakeWebSocket.last!.simMessage(rpcResponse(resumeReq.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(controller.getState().restoredThreadId).toBe("hthread-1"))

    // The restore path also fires command/list (fire-and-forget); respond with
    // a snapshot and assert it lands in `commands`.
    const commandReq = FakeWebSocket.last!.sent
      .map((raw) => JSON.parse(raw))
      .find((m) => m.method === "command/list")!
    expect(commandReq).toBeDefined()
    FakeWebSocket.last!.simMessage(
      rpcResponse(commandReq.id, {
        data: [
          {
            name: "compact",
            aliases: [],
            description: "Summarize history.",
            kind: "control",
            source: "builtin",
            controlAction: "compact",
          },
        ],
      }),
    )
    await flush()

    await vi.waitFor(() => expect(controller.getState().commands).toHaveLength(1))
    expect(controller.getState().commands[0]).toMatchObject({ name: "compact", kind: "control" })
  })

  it("maps user messages to their turn index after restore", async () => {
    const controller = makeController("s1")
    controller.start()
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(controller.getState().restoredThreadId).toBe("hthread-1"))
    // THREAD's single user message (u1) lives in turn 0.
    expect(controller.getState().userMessageTurnIndex.get("u1")).toBe(0)
  })

  // ── authoritative thread status + steering (S7) ───────────────────────────

  /** Restore a bound thread and return the settled controller + socket. */
  async function restoredController(): Promise<{
    controller: ConversationController
    socket: FakeWebSocket
  }> {
    const controller = makeController("s1")
    controller.start()
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(controller.getState().restoredThreadId).toBe("hthread-1"))
    return { controller, socket: FakeWebSocket.last! }
  }

  function findRequest(socket: FakeWebSocket, method: string): { id: number | string } {
    const req = socket.sent.map((raw) => JSON.parse(raw)).find((m) => m.method === method)
    if (!req) throw new Error(`no ${method} request was sent`)
    return req
  }

  /** Count outbound requests with the given method (e.g. resync `thread/resume`s). */
  function countRequests(socket: FakeWebSocket, method: string): number {
    return socket.sent.filter((raw) => JSON.parse(raw).method === method).length
  }

  it("tracks the authoritative thread status and ignores other threads", async () => {
    const { controller, socket } = await restoredController()

    socket.simMessage(
      notification(HARNESS_NOTIFICATION.THREAD_STATUS_CHANGED, {
        threadId: "hthread-1",
        status: "running",
      }),
    )
    await flush()
    expect(controller.getState().threadStatus).toBe("running")

    // Another thread's status must not leak into this conversation.
    socket.simMessage(
      notification(HARNESS_NOTIFICATION.THREAD_STATUS_CHANGED, {
        threadId: "hthread-other",
        status: "interrupted",
      }),
    )
    await flush()
    expect(controller.getState().threadStatus).toBe("running")

    socket.simMessage(
      notification(HARNESS_NOTIFICATION.THREAD_STATUS_CHANGED, {
        threadId: "hthread-1",
        status: "interrupted",
      }),
    )
    await flush()
    expect(controller.getState().threadStatus).toBe("interrupted")
    controller.dispose()
  })

  it("sendSteering queues on a running turn and the terminal event clears it", async () => {
    const { controller, socket } = await restoredController()

    socket.simMessage(
      notification(HARNESS_NOTIFICATION.THREAD_STATUS_CHANGED, {
        threadId: "hthread-1",
        status: "running",
      }),
    )
    await flush()

    const sendPromise = controller.sendSteering({
      id: "steer-1",
      role: "user",
      parts: [{ type: "text", text: "also check the tests" }],
    })
    await flush()
    const turnReq = findRequest(socket, "turn/start")
    socket.simMessage(
      rpcResponse(turnReq.id, { turn: { id: "0", status: "queued" }, queued: true }),
    )
    const result = await sendPromise
    expect(result.queued).toBe(true)
    expect(controller.getState().queuedCount).toBe(1)
    expect(controller.getState().queuedTexts).toEqual(["also check the tests"])

    // The run's terminal event carries the abort reason and clears the queue…
    socket.simMessage(
      notification(HARNESS_NOTIFICATION.TURN_COMPLETED, {
        threadId: "hthread-1",
        turn: { id: "1", status: "interrupted" },
        reason: "max_turns_reached",
      }),
    )
    await flush()
    expect(controller.getState().abortReason).toBe("max_turns_reached")
    expect(controller.getState().queuedCount).toBe(0)
    // …and, because something WAS queued, refreshes the history so the drained
    // input materializes as a real row (the live stream never replays it).
    await vi.waitFor(() => expect(countRequests(socket, "thread/resume")).toBe(2))
    const resyncReq = socket.sent
      .map((raw) => JSON.parse(raw))
      .filter((m) => m.method === "thread/resume")
      .at(-1)
    socket.simMessage(rpcResponse(resyncReq.id, { thread: THREAD }))
    await vi.waitFor(() => expect(controller.getState().isHistoryLoading).toBe(false))

    // A clean completion clears the abort reason again.
    socket.simMessage(
      notification(HARNESS_NOTIFICATION.TURN_COMPLETED, {
        threadId: "hthread-1",
        turn: { id: "2", status: "completed" },
        reason: "completed",
      }),
    )
    await flush()
    expect(controller.getState().abortReason).toBeNull()
    controller.dispose()
  })

  it("a lost steering race defers the history refresh to the run's terminal event", async () => {
    const { controller, socket } = await restoredController()

    socket.simMessage(
      notification(HARNESS_NOTIFICATION.THREAD_STATUS_CHANGED, {
        threadId: "hthread-1",
        status: "running",
      }),
    )
    await flush()

    // The server reports the input STARTED a new run (not queued): no local
    // stream is subscribed to it, so the refresh must wait for its end.
    const sendPromise = controller.sendSteering({
      id: "steer-race",
      role: "user",
      parts: [{ type: "text", text: "orphaned run" }],
    })
    await flush()
    const turnReq = findRequest(socket, "turn/start")
    socket.simMessage(rpcResponse(turnReq.id, { turn: { id: "0", status: "in_progress" } }))
    await sendPromise
    await flush()
    expect(countRequests(socket, "thread/resume")).toBe(1)

    socket.simMessage(
      notification(HARNESS_NOTIFICATION.THREAD_STATUS_CHANGED, {
        threadId: "hthread-1",
        status: "completed",
      }),
    )
    await vi.waitFor(() => expect(countRequests(socket, "thread/resume")).toBe(2))
    controller.dispose()
  })

  it("an interrupt with queued input resyncs on the terminal status event", async () => {
    const { controller, socket } = await restoredController()

    socket.simMessage(
      notification(HARNESS_NOTIFICATION.THREAD_STATUS_CHANGED, {
        threadId: "hthread-1",
        status: "running",
      }),
    )
    await flush()

    const sendPromise = controller.sendSteering({
      id: "steer-3",
      role: "user",
      parts: [{ type: "text", text: "stop early" }],
    })
    await flush()
    const turnReq = findRequest(socket, "turn/start")
    socket.simMessage(rpcResponse(turnReq.id, { turn: { id: "0", status: "queued" }, queued: true }))
    await sendPromise
    expect(controller.getState().queuedCount).toBe(1)

    const interrupting = controller.interrupt()
    await flush()
    const interruptReq = findRequest(socket, "turn/interrupt")
    socket.simMessage(rpcResponse(interruptReq.id, {}))
    await interrupting
    expect(controller.getState().queuedCount).toBe(0)
    // Teardown persisted the undelivered input; the terminal event materializes it.
    socket.simMessage(
      notification(HARNESS_NOTIFICATION.THREAD_STATUS_CHANGED, {
        threadId: "hthread-1",
        status: "interrupted",
      }),
    )
    await vi.waitFor(() => expect(countRequests(socket, "thread/resume")).toBe(2))
    controller.dispose()
  })

  it("interrupt clears the queued-steering display", async () => {
    const { controller, socket } = await restoredController()

    socket.simMessage(
      notification(HARNESS_NOTIFICATION.THREAD_STATUS_CHANGED, {
        threadId: "hthread-1",
        status: "running",
      }),
    )
    await flush()

    const sendPromise = controller.sendSteering({
      id: "steer-2",
      role: "user",
      parts: [{ type: "text", text: "stop early" }],
    })
    await flush()
    const turnReq = findRequest(socket, "turn/start")
    socket.simMessage(
      rpcResponse(turnReq.id, { turn: { id: "0", status: "queued" }, queued: true }),
    )
    await sendPromise
    expect(controller.getState().queuedCount).toBe(1)

    const interrupting = controller.interrupt()
    await flush()
    const interruptReq = findRequest(socket, "turn/interrupt")
    socket.simMessage(rpcResponse(interruptReq.id, {}))
    await interrupting
    expect(controller.getState().queuedCount).toBe(0)
    controller.dispose()
  })

  it("retracts a turn via thread/rollback and re-resumes the thread", async () => {
    const controller = makeController("s1")
    controller.start()
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(controller.getState().restoredThreadId).toBe("hthread-1"))

    const emptied: Thread = { ...THREAD, turns: [] }
    const p = controller.rollbackFromTurn(2)
    await flush()
    // thread/rollback keeps turns 0..turnIndex-1 (toTurnId = "1").
    const rbReq = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    expect(rbReq.method).toBe("thread/rollback")
    expect(rbReq.params).toMatchObject({ threadId: "hthread-1", toTurnId: "1" })
    FakeWebSocket.last!.simMessage(rpcResponse(rbReq.id, { thread: emptied }))
    await flush()
    // then a re-resume refreshes the message list.
    const resumeReq = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    expect(resumeReq.method).toBe("thread/resume")
    FakeWebSocket.last!.simMessage(rpcResponse(resumeReq.id, { thread: emptied }))
    await flush()
    await p

    await vi.waitFor(() => expect(controller.getState().isRollingBack).toBe(false))
    expect(controller.getState().error).toBeNull()
    expect(controller.getState().restoredMessages).toHaveLength(0)
  })

  it("treats rollback for turn 0 as a no-op (nothing before it to keep)", async () => {
    const controller = makeController("s1")
    controller.start()
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(controller.getState().restoredThreadId).toBe("hthread-1"))

    const sentBefore = FakeWebSocket.last!.sent.length
    await controller.rollbackFromTurn(0)
    expect(FakeWebSocket.last!.sent.length).toBe(sentBefore)
  })

  it("retries the transport open when the first dial fails, then restores", async () => {
    const controller = makeController("s1")
    controller.start()
    await flush()
    // First dial fails transiently (e.g. slab-server not yet ready).
    FakeWebSocket.last!.simError()
    await flush()
    // The controller backs off, then redials on a fresh socket.
    await new Promise((resolve) => setTimeout(resolve, RESTORE_BACKOFF_MS + 120))
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()

    await vi.waitFor(() => expect(controller.getState().restoredThreadId).toBe("hthread-1"))
    expect(controller.getState().error).toBeNull()
  })

  it("surfaces a restore error only after exhausting the open retries", async () => {
    const controller = makeController("s1")
    controller.start()
    // Attempts 1..MAX-1 fail and back off (linear) before redialing.
    for (let attempt = 1; attempt < MAX_RESTORE_ATTEMPTS; attempt += 1) {
      await flush() // let the next dial create its socket
      FakeWebSocket.last!.simError()
      await flush() // let the rejection propagate
      await new Promise((resolve) => setTimeout(resolve, RESTORE_BACKOFF_MS * attempt + 120))
    }
    // Still mid-retry, before the final attempt: no error surfaced yet.
    expect(controller.getState().error).toBeNull()

    // Final attempt fails → the error is now surfaced.
    await flush()
    FakeWebSocket.last!.simError()
    await flush()
    await vi.waitFor(() => expect(controller.getState().error).toBeTruthy())
    expect(controller.getState().isHistoryLoading).toBe(false)
  })

  it("compacts the current thread and refreshes the compacted history", async () => {
    const controller = makeController("s1")
    controller.start()
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(controller.getState().restoredThreadId).toBe("hthread-1"))

    const compacted: Thread = {
      ...THREAD,
      turns: [{ ...THREAD.turns[0], items: [THREAD.turns[0].items[0]] }],
    }
    const p = controller.compactThread()
    await flush()
    const startReq = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    expect(startReq.method).toBe("thread/compact/start")
    FakeWebSocket.last!.simMessage(rpcResponse(startReq.id, {}))
    await flush()
    const resumeReq = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    expect(resumeReq.method).toBe("thread/resume")
    FakeWebSocket.last!.simMessage(rpcResponse(resumeReq.id, { thread: compacted }))
    await flush()
    await p

    await vi.waitFor(() => expect(controller.getState().isCompacting).toBe(false))
    expect(controller.getState().actionError).toBeNull()
    expect(controller.getState().error).toBeNull()
    expect(controller.getState().restoredMessages).toHaveLength(1)
    expect(
      controller.getState().compactionMarkers.some(
        (m) => m.mode === "manual" && m.phase === "compacted",
      ),
    ).toBe(true)
  })

  it("surfaces a compact rejection as an action error (separate from restore errors)", async () => {
    const controller = makeController("s1")
    controller.start()
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(controller.getState().restoredThreadId).toBe("hthread-1"))

    const p = controller.compactThread()
    await flush()
    const startReq = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    expect(startReq.method).toBe("thread/compact/start")
    FakeWebSocket.last!.simMessage(
      rpcError(startReq.id, "thread is running; interrupt it before compacting"),
    )
    await flush()
    await p

    await vi.waitFor(() => expect(controller.getState().actionError?.kind).toBe("compact"))
    expect(controller.getState().actionError?.message).toContain("thread is running")
    expect(
      controller.getState().compactionMarkers.some((m) => m.mode === "manual"),
    ).toBe(false)
    expect(controller.getState().isCompacting).toBe(false)
    // Restore error stays clear — action errors are surfaced separately.
    expect(controller.getState().error).toBeNull()
  })

  it("surfaces a compact with no bound thread as an action error without an RPC", async () => {
    const controller = makeController("s1")
    controller.start()
    await driveOpenAndInit()
    // Fresh session → no thread bound (currentThreadId stays null).
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcError(req.id, "no thread to resume for session"))
    await flush()
    await vi.waitFor(() => expect(controller.getState().isHistoryLoading).toBe(false))
    expect(controller.getState().restoredThreadId).toBeNull()

    const sentBefore = FakeWebSocket.last!.sent.length
    await controller.compactThread()
    expect(controller.getState().actionError?.kind).toBe("compact")
    expect(FakeWebSocket.last!.sent.length).toBe(sentBefore)
  })

  // ── new API surface ──────────────────────────────────────────────────────

  it("send() opens, lazily binds a thread, and starts the turn with the shared input mapping", async () => {
    const controller = new ConversationController({
      sessionId: "s1",
      model: "m1",
      WebSocketCtor: FakeWebSocket as unknown as typeof WebSocket,
    })

    const p = controller.send({
      id: "m-1",
      role: "user",
      parts: [{ type: "text", text: "hello" }],
    })
    await driveOpenAndInit()

    // No thread bound → thread/start fires first.
    const startReq = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    expect(startReq.method).toBe("thread/start")
    FakeWebSocket.last!.simMessage(
      rpcResponse(startReq.id, { thread: { id: "hthread-new", preview: "", modelProvider: "", createdAt: 0, turns: [] } }),
    )
    await flush()

    const turnReq = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    expect(turnReq.method).toBe("turn/start")
    expect(turnReq.params).toMatchObject({
      threadId: "hthread-new",
      model: "m1",
      input: [{ type: "text", text: "hello", textElements: [] }],
    })
    FakeWebSocket.last!.simMessage(rpcResponse(turnReq.id, { turn: { id: "1" } }))
    await expect(p).resolves.toMatchObject({ turn: { id: "1" } })
    expect(controller.client.currentThreadId).toBe("hthread-new")
  })

  it("interrupt() sends turn/interrupt for the bound thread and no-ops without one", async () => {
    const controller = makeController("s1")
    controller.start()
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(controller.getState().restoredThreadId).toBe("hthread-1"))

    const p = controller.interrupt()
    await flush()
    const interruptReq = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    expect(interruptReq.method).toBe("turn/interrupt")
    expect(interruptReq.params).toMatchObject({ threadId: "hthread-1", turnId: "0" })
    FakeWebSocket.last!.simMessage(rpcResponse(interruptReq.id, {}))
    await expect(p).resolves.toBeUndefined()

    // Unbound controller → no RPC at all.
    const bare = makeController()
    const sentBefore = FakeWebSocket.last!.sent.length
    await bare.interrupt()
    expect(FakeWebSocket.last!.sent.length).toBe(sentBefore)
  })

  it("reconnect() re-runs the restore machine on demand", async () => {
    const controller = makeController("s1")
    controller.start()
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(controller.getState().restoredThreadId).toBe("hthread-1"))

    // A manual reconnect re-issues thread/resume and refreshes the projection.
    const refreshed: Thread = { ...THREAD, turns: [] }
    const reconnected = controller.reconnect()
    await flush()
    const resumeReq = FakeWebSocket.last!.sent
      .map((raw) => JSON.parse(raw))
      .filter((m) => m.method === "thread/resume")
      .at(-1)!
    FakeWebSocket.last!.simMessage(rpcResponse(resumeReq.id, { thread: refreshed }))
    await reconnected
    await vi.waitFor(() => expect(controller.getState().restoredMessages).toHaveLength(0))
    expect(controller.getState().restoredThreadId).toBe("hthread-1")
  })

  it("dispose() closes the client and stops in-flight restore work from landing", async () => {
    const controller = makeController("s1")
    controller.start()
    await driveOpenAndInit()
    // thread/resume is in flight — dispose before answering it.
    expect(FakeWebSocket.last!.sent.some((raw) => raw.includes("thread/resume"))).toBe(true)
    const resumeReq = FakeWebSocket.last!.sent
      .map((raw) => JSON.parse(raw))
      .find((m) => m.method === "thread/resume")!

    controller.dispose()
    expect(controller.client.getStatus()).toBe("closed")

    // The late resume response must not land (stale generation).
    FakeWebSocket.last!.simMessage(rpcResponse(resumeReq.id, { thread: THREAD }))
    await flush()
    expect(controller.getState().restoredThreadId).toBeNull()

    // Idempotent.
    controller.dispose()
  })

  it("treats an undefined session as a fresh projection with a version bump", async () => {
    const controller = makeController(undefined)
    controller.start()
    await flush()
    expect(controller.getState().restoreVersion).toBe(1)
    expect(controller.getState().restoredThreadId).toBeNull()
    expect(controller.getState().activeConversation).toBeUndefined()
    expect(controller.client.currentThreadId).toBeNull()
    expect(controller.client.lastTurnIndex).toBe(-1)
    // No socket is ever dialed for a session-less controller.
    expect(FakeWebSocket.last).toBeUndefined()
  })
})

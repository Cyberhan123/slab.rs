import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import { renderHook } from "vitest-browser-react"

import type { Thread } from "@slab/core/harness"
import { FakeWebSocket } from "../../lib/__tests__/fake-websocket"
import {
  MAX_RESTORE_ATTEMPTS,
  RESTORE_BACKOFF_MS,
  useHarnessConversation,
} from "../use-harness-conversation"

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

  it("restores a resumed thread into messages and binds the thread id", async () => {
    const { result, unmount } = await renderHook(() => useHarnessConversation("s1", "m1"))

    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()

    await vi.waitFor(() => {
      expect(result.current.isHistoryLoading).toBe(false)
    })
    expect(result.current.restoredThreadId).toBe("hthread-1")
    expect(result.current.activeConversation).toBe("s1")
    expect(result.current.restoredMessages).toHaveLength(2)
    expect(result.current.error).toBeNull()
    await unmount()
  })

  it("treats a 'no thread to resume' rejection as a fresh session", async () => {
    const { result, unmount } = await renderHook(() => useHarnessConversation("s2", "m1"))

    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcError(req.id, "no thread to resume for session"))
    await flush()

    await vi.waitFor(() => {
      expect(result.current.isHistoryLoading).toBe(false)
    })
    expect(result.current.restoredThreadId).toBeNull()
    expect(result.current.restoredMessages).toHaveLength(0)
    expect(result.current.error).toBeNull()
    await unmount()
  })

  it("surfaces an unexpected resume error", async () => {
    const { result, unmount } = await renderHook(() => useHarnessConversation("s3", "m1"))

    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcError(req.id, "internal boom"))
    await flush()

    await vi.waitFor(() => {
      expect(result.current.error).toContain("internal boom")
    })
    expect(result.current.isHistoryLoading).toBe(false)
    await unmount()
  })

  it("tracks a command-execution approval request and ignores other threads", async () => {
    const { result, unmount } = await renderHook(() => useHarnessConversation("s1", "m1"))
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(result.current.restoredThreadId).toBe("hthread-1"))

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
    await vi.waitFor(() => expect(result.current.approvals).toHaveLength(1))
    expect(result.current.approvals[0]).toMatchObject({ itemId: "call-1", command: "echo hi", status: "pending" })
    expect(result.current.approvalStatusByItemId.get("call-1")).toBe("pending")

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
    expect(result.current.approvals).toHaveLength(1)
    await unmount()
  })

  it("resolves an approval optimistically and keeps it approved when delivered", async () => {
    const { result, act, unmount } = await renderHook(() => useHarnessConversation("s1", "m1"))
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(result.current.restoredThreadId).toBe("hthread-1"))
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
    await vi.waitFor(() => expect(result.current.approvals).toHaveLength(1))

    let p!: Promise<void>
    await act(async () => {
      p = result.current.resolveApproval("call-1", true, "run_once")
      await flush()
    })
    const resolveReq = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    expect(resolveReq.method).toBe("approval/resolve")
    await act(async () => {
      FakeWebSocket.last!.simMessage(rpcResponse(resolveReq.id, { delivered: true, status: "approved" }))
      await flush()
    })
    await expect(p).resolves.toBeUndefined()
    // Approved ⇒ no longer in the pending list, but status map records it.
    await vi.waitFor(() => expect(result.current.approvals).toHaveLength(0))
    expect(result.current.approvalStatusByItemId.get("call-1")).toBe("approved")
    await unmount()
  })

  it("reverts to pending and rejects when delivery failed (delivered=false)", async () => {
    const { result, act, unmount } = await renderHook(() => useHarnessConversation("s1", "m1"))
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(result.current.restoredThreadId).toBe("hthread-1"))
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
    await vi.waitFor(() => expect(result.current.approvals).toHaveLength(1))

    // Capture the rejection as a resolution immediately (attached before the
    // response arrives) so it is never momentarily unhandled, and so the
    // assertion stays lint-clean (no un-awaited `.rejects`).
    let captured: unknown
    let p!: Promise<void>
    await act(async () => {
      p = result.current.resolveApproval("call-1", true, "run_once")
      await flush()
    })
    const capturedPromise = p.then(
      () => new Error("expected resolveApproval to reject, but it resolved"),
      (err: unknown) => err,
    )
    const resolveReq = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    await act(async () => {
      FakeWebSocket.last!.simMessage(rpcResponse(resolveReq.id, { delivered: false }))
      await flush()
    })
    captured = await capturedPromise
    expect(String(captured)).toContain("approval not delivered")
    await vi.waitFor(() =>
      expect(result.current.approvalStatusByItemId.get("call-1")).toBe("pending"),
    )
    expect(result.current.approvals).toHaveLength(1)
    await unmount()
  })

  it("accumulates live command output under the 256 KiB per-item cap", async () => {
    const { result, unmount } = await renderHook(() => useHarnessConversation("s1", "m1"))
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(result.current.restoredThreadId).toBe("hthread-1"))

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
      expect(result.current.liveOutputByItemId.get("call-1")).toBe("part-1\npart-2\n"),
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
    expect(result.current.liveOutputByItemId.get("call-1")).toBe("part-1\npart-2\n")

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
    expect(result.current.liveOutputByItemId.get("call-1")).toBe("part-1\npart-2\n")
    await unmount()
  })

  it("resets approval + live-output state when the session changes", async () => {
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
    await vi.waitFor(() => expect(result.current.approvals).toHaveLength(1))

    const beforeVersion = result.current.restoreVersion
    // Switch to a fresh session: state must reset and restoreVersion bump.
    await rerender({ sid: undefined })
    await vi.waitFor(() => expect(result.current.restoreVersion).not.toBe(beforeVersion))
    expect(result.current.approvals).toHaveLength(0)
    expect(result.current.liveOutputByItemId.size).toBe(0)
    await unmount()
  })

  it("forks the current thread and rebinds the socket to the child", async () => {
    const { result, act, unmount } = await renderHook(() => useHarnessConversation("s1", "m1"))
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(result.current.restoredThreadId).toBe("hthread-1"))

    const CHILD: Thread = { ...THREAD, id: "hthread-child", createdAt: 123 }

    let p!: Promise<void>
    await act(async () => {
      p = result.current.forkThread()
      await flush()
    })
    // thread/fork → returns the child thread (the hook only consumes child.id).
    const forkReq = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    expect(forkReq.method).toBe("thread/fork")
    await act(async () => {
      FakeWebSocket.last!.simMessage(rpcResponse(forkReq.id, { thread: CHILD }))
      await flush()
    })
    // thread/resume of the child → returns the child's copied history.
    const resumeReq = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    expect(resumeReq.method).toBe("thread/resume")
    expect(resumeReq.params).toMatchObject({ threadId: "hthread-child" })
    await act(async () => {
      FakeWebSocket.last!.simMessage(rpcResponse(resumeReq.id, { thread: CHILD }))
      await flush()
      await p
    })

    await vi.waitFor(() => expect(result.current.restoredThreadId).toBe("hthread-child"))
    expect(result.current.isForking).toBe(false)
    expect(result.current.error).toBeNull()
    expect(result.current.restoredMessages).toHaveLength(2)
    await unmount()
  })

  it("fetches the command registry on restore and exposes it as commands", async () => {
    const { result, unmount } = await renderHook(() => useHarnessConversation("s1", "m1"))
    await driveOpenAndInit()

    // Complete thread/resume so the restore path settles.
    const resumeReq = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    expect(resumeReq.method).toBe("thread/resume")
    FakeWebSocket.last!.simMessage(rpcResponse(resumeReq.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(result.current.restoredThreadId).toBe("hthread-1"))

    // The restore effect also fires command/list (fire-and-forget); respond with
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

    await vi.waitFor(() => expect(result.current.commands).toHaveLength(1))
    expect(result.current.commands[0]).toMatchObject({ name: "compact", kind: "control" })
    await unmount()
  })

  it("maps user messages to their turn index after restore", async () => {
    const { result, unmount } = await renderHook(() => useHarnessConversation("s1", "m1"))
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(result.current.restoredThreadId).toBe("hthread-1"))
    // THREAD's single user message (u1) lives in turn 0.
    expect(result.current.userMessageTurnIndex.get("u1")).toBe(0)
    await unmount()
  })

  it("retracts a turn via thread/rollback and re-resumes the thread", async () => {
    const { result, act, unmount } = await renderHook(() => useHarnessConversation("s1", "m1"))
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(result.current.restoredThreadId).toBe("hthread-1"))

    const emptied: Thread = { ...THREAD, turns: [] }
    let p!: Promise<void>
    await act(async () => {
      p = result.current.rollbackFromTurn(2)
      await flush()
    })
    // thread/rollback keeps turns 0..turnIndex-1 (toTurnId = "1").
    const rbReq = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    expect(rbReq.method).toBe("thread/rollback")
    expect(rbReq.params).toMatchObject({ threadId: "hthread-1", toTurnId: "1" })
    await act(async () => {
      FakeWebSocket.last!.simMessage(rpcResponse(rbReq.id, { thread: emptied }))
      await flush()
    })
    // then a re-resume refreshes the message list.
    const resumeReq = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    expect(resumeReq.method).toBe("thread/resume")
    await act(async () => {
      FakeWebSocket.last!.simMessage(rpcResponse(resumeReq.id, { thread: emptied }))
      await flush()
      await p
    })

    await vi.waitFor(() => expect(result.current.isRollingBack).toBe(false))
    expect(result.current.error).toBeNull()
    expect(result.current.restoredMessages).toHaveLength(0)
    await unmount()
  })

  it("treats rollback for turn 0 as a no-op (nothing before it to keep)", async () => {
    const { result, unmount } = await renderHook(() => useHarnessConversation("s1", "m1"))
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(result.current.restoredThreadId).toBe("hthread-1"))

    const sentBefore = FakeWebSocket.last!.sent.length
    await result.current.rollbackFromTurn(0)
    expect(FakeWebSocket.last!.sent.length).toBe(sentBefore)
    await unmount()
  })

  it("retries the transport open when the first dial fails, then restores", async () => {
    const { result, unmount } = await renderHook(() => useHarnessConversation("s1", "m1"))
    await flush()
    // First dial fails transiently (e.g. slab-server not yet ready).
    FakeWebSocket.last!.simError()
    await flush()
    // The hook backs off, then redials on a fresh socket.
    await new Promise((resolve) => setTimeout(resolve, RESTORE_BACKOFF_MS + 120))
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()

    await vi.waitFor(() => expect(result.current.restoredThreadId).toBe("hthread-1"))
    expect(result.current.error).toBeNull()
    await unmount()
  })

  it("surfaces a restore error only after exhausting the open retries", async () => {
    const { result, unmount } = await renderHook(() => useHarnessConversation("s1", "m1"))
    // Attempts 1..MAX-1 fail and back off (linear) before redialing.
    for (let attempt = 1; attempt < MAX_RESTORE_ATTEMPTS; attempt += 1) {
      await flush() // let the next dial create its socket
      FakeWebSocket.last!.simError()
      await flush() // let the rejection propagate
      await new Promise((resolve) => setTimeout(resolve, RESTORE_BACKOFF_MS * attempt + 120))
    }
    // Still mid-retry, before the final attempt: no error surfaced yet.
    expect(result.current.error).toBeNull()

    // Final attempt fails → the error is now surfaced.
    await flush()
    FakeWebSocket.last!.simError()
    await flush()
    await vi.waitFor(() => expect(result.current.error).toBeTruthy())
    expect(result.current.isHistoryLoading).toBe(false)
    await unmount()
  })

  it("compacts the current thread and refreshes the compacted history", async () => {
    const { result, act, unmount } = await renderHook(() => useHarnessConversation("s1", "m1"))
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(result.current.restoredThreadId).toBe("hthread-1"))

    const compacted: Thread = {
      ...THREAD,
      turns: [{ ...THREAD.turns[0], items: [THREAD.turns[0].items[0]] }],
    }
    let p!: Promise<void>
    await act(async () => {
      p = result.current.compactThread()
      await flush()
    })
    const startReq = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    expect(startReq.method).toBe("thread/compact/start")
    await act(async () => {
      FakeWebSocket.last!.simMessage(rpcResponse(startReq.id, {}))
      await flush()
    })
    const resumeReq = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    expect(resumeReq.method).toBe("thread/resume")
    await act(async () => {
      FakeWebSocket.last!.simMessage(rpcResponse(resumeReq.id, { thread: compacted }))
      await flush()
      await p
    })

    await vi.waitFor(() => expect(result.current.isCompacting).toBe(false))
    expect(result.current.actionError).toBeNull()
    expect(result.current.error).toBeNull()
    expect(result.current.restoredMessages).toHaveLength(1)
    expect(
      result.current.compactionMarkers.some((m) => m.mode === "manual" && m.phase === "compacted"),
    ).toBe(true)
    await unmount()
  })

  it("surfaces a compact rejection as an action error (separate from restore errors)", async () => {
    const { result, act, unmount } = await renderHook(() => useHarnessConversation("s1", "m1"))
    await driveOpenAndInit()
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcResponse(req.id, { thread: THREAD }))
    await flush()
    await vi.waitFor(() => expect(result.current.restoredThreadId).toBe("hthread-1"))

    let p!: Promise<void>
    await act(async () => {
      p = result.current.compactThread()
      await flush()
    })
    const startReq = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    expect(startReq.method).toBe("thread/compact/start")
    await act(async () => {
      FakeWebSocket.last!.simMessage(
        rpcError(startReq.id, "thread is running; interrupt it before compacting"),
      )
      await flush()
      await p
    })

    await vi.waitFor(() => expect(result.current.actionError?.kind).toBe("compact"))
    expect(result.current.actionError?.message).toContain("thread is running")
    expect(result.current.compactionMarkers.some((m) => m.mode === "manual")).toBe(false)
    expect(result.current.isCompacting).toBe(false)
    // Restore error stays clear — action errors are surfaced separately.
    expect(result.current.error).toBeNull()
    await unmount()
  })

  it("surfaces a compact with no bound thread as an action error without an RPC", async () => {
    const { result, act, unmount } = await renderHook(() => useHarnessConversation("s1", "m1"))
    await driveOpenAndInit()
    // Fresh session → no thread bound (currentThreadId stays null).
    const req = JSON.parse(FakeWebSocket.last!.sent.at(-1)!)
    FakeWebSocket.last!.simMessage(rpcError(req.id, "no thread to resume for session"))
    await flush()
    await vi.waitFor(() => expect(result.current.isHistoryLoading).toBe(false))
    expect(result.current.restoredThreadId).toBeNull()

    const sentBefore = FakeWebSocket.last!.sent.length
    await act(async () => {
      await result.current.compactThread()
    })
    expect(result.current.actionError?.kind).toBe("compact")
    expect(FakeWebSocket.last!.sent.length).toBe(sentBefore)
    await unmount()
  })
})

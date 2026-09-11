import { afterEach, describe, expect, it, vi } from "vitest"

import { POOL_IDLE_DISPOSE_MS, conversationPool } from "../conversation-pool"
import { ConversationController } from "../conversation-controller"

vi.useFakeTimers()

describe("conversationPool", () => {
  afterEach(() => {
    vi.clearAllTimers()
    conversationPool.disposeAll()
  })

  it("returns the same controller for the same session", () => {
    const first = conversationPool.acquire("s1")
    const second = conversationPool.acquire("s1")
    expect(second).toBe(first)
  })

  it("mints distinct controllers per session", () => {
    const a = conversationPool.acquire("s1")
    const b = conversationPool.acquire("s2")
    expect(a).not.toBe(b)
  })

  it("release does not dispose; the idle timer does", () => {
    const controller = conversationPool.acquire("s1")
    const disposeSpy = vi.spyOn(controller, "dispose")
    conversationPool.release(controller)
    expect(disposeSpy).not.toHaveBeenCalled()

    vi.advanceTimersByTime(POOL_IDLE_DISPOSE_MS - 1)
    expect(disposeSpy).not.toHaveBeenCalled()

    vi.advanceTimersByTime(1)
    expect(disposeSpy).toHaveBeenCalledTimes(1)
    // The entry is gone: a re-acquire mints a fresh controller.
    expect(conversationPool.acquire("s1")).not.toBe(controller)
  })

  it("a re-acquire before the idle window cancels disposal", () => {
    const controller = conversationPool.acquire("s1")
    conversationPool.release(controller)
    vi.advanceTimersByTime(POOL_IDLE_DISPOSE_MS - 1)
    const revived = conversationPool.acquire("s1")
    expect(revived).toBe(controller)

    const disposeSpy = vi.spyOn(controller, "dispose")
    vi.advanceTimersByTime(POOL_IDLE_DISPOSE_MS * 2)
    // Still alive: the release's timer was cancelled by the acquire.
    expect(disposeSpy).not.toHaveBeenCalled()
  })

  it("release of a superseded controller arms nothing for the live entry", () => {
    const old = conversationPool.acquire("s1")
    conversationPool.release(old)
    vi.advanceTimersByTime(POOL_IDLE_DISPOSE_MS)
    // `old` was disposed by the timer and dropped from the pool; a stale
    // release of it must not affect the new entry.
    const fresh = conversationPool.acquire("s1")
    const freshDispose = vi.spyOn(fresh, "dispose")
    conversationPool.release(old)
    vi.advanceTimersByTime(POOL_IDLE_DISPOSE_MS * 2)
    expect(freshDispose).not.toHaveBeenCalled()
  })

  it("keeps an acquired controller across release/acquire cycles without disposing", () => {
    const controller = conversationPool.acquire("s1")
    const disposeSpy = vi.spyOn(controller, "dispose")
    for (let i = 0; i < 5; i += 1) {
      conversationPool.release(controller)
      vi.advanceTimersByTime(1000)
      expect(conversationPool.acquire("s1")).toBe(controller)
    }
    expect(disposeSpy).not.toHaveBeenCalled()
  })
})

describe("ConversationController.sessionId", () => {
  it("is exposed for pool keying", () => {
    const controller = new ConversationController({ sessionId: "pool-key" })
    expect(controller.sessionId).toBe("pool-key")
    controller.dispose()
  })
})

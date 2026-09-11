/**
 * Process-wide keep-alive pool for {@link ConversationController}s.
 *
 * The assistant page used to own its controllers (`useMemo` per sessionId,
 * `dispose()` on unmount): navigating away from the detail view closed the
 * harness WS mid-run, and coming back reconnected to a thread whose live
 * stream could no longer be reattached — the user experienced "the task was
 * interrupted" although the server kept running it. The pool keeps a
 * controller (and its socket) alive across route changes and disposes it only
 * after it has been idle for {@link POOL_IDLE_DISPOSE_MS}, so:
 *   - switching pages keeps the notification feed (and any in-flight turn)
 *     alive server-side AND client-side;
 *   - returning re-`acquire`s the SAME controller — same mirrors, same
 *     live-resume snapshot, same restoreVersion;
 *   - a controller nobody looks at still tears down (sockets are not free).
 */

import { ConversationController } from "./conversation-controller"

/** Idle window before an unacquired controller is disposed. */
const POOL_IDLE_DISPOSE_MS = 5 * 60 * 1000

interface PoolEntry {
  controller: ConversationController
  idleTimer: ReturnType<typeof setTimeout> | null
}

class ConversationControllerPool {
  private readonly entries = new Map<string, PoolEntry>()

  /**
   * Get (or create) the controller for `sessionId`. A pooled controller is
   * revived in place: its pending idle disposal is cancelled and `start()`
   * (idempotent) re-arms its subscriptions after a dispose-free idle window
   * has passed without disposal.
   */
  acquire(sessionId: string | undefined): ConversationController {
    const key = sessionId ?? ""
    const existing = this.entries.get(key)
    if (existing) {
      if (existing.idleTimer !== null) {
        clearTimeout(existing.idleTimer)
        existing.idleTimer = null
      }
      existing.controller.start()
      return existing.controller
    }
    const controller = new ConversationController({ sessionId })
    this.entries.set(key, { controller, idleTimer: null })
    return controller
  }

  /**
   * Return a controller to the pool (page unmount). Does NOT dispose: an
   * idle timer owns the eventual teardown, cancelled by a matching
   * `acquire`. A controller that never re-acquires is disposed after
   * {@link POOL_IDLE_DISPOSE_MS}.
   */
  release(controller: ConversationController): void {
    const key = controller.sessionId ?? ""
    const entry = this.entries.get(key)
    // The entry may already be gone (a concurrent dispose raced) or replaced
    // by a newer controller for the same session — only the LIVE entry's
    // timer may be armed.
    if (!entry || entry.controller !== controller) return
    if (entry.idleTimer !== null) return
    entry.idleTimer = setTimeout(() => {
      // Identity check: a re-acquire after this timer fired but before the
      // callback ran must not dispose the revived controller.
      if (this.entries.get(key)?.controller !== controller) return
      this.entries.delete(key)
      controller.dispose()
    }, POOL_IDLE_DISPOSE_MS)
  }

  /** Test seam: drop every pooled controller immediately. */
  disposeAll(): void {
    for (const [, entry] of this.entries) {
      if (entry.idleTimer !== null) clearTimeout(entry.idleTimer)
      entry.controller.dispose()
    }
    this.entries.clear()
  }
}

/** Module-level singleton: the pool outlives every page/route instance. */
export const conversationPool = new ConversationControllerPool()

export { POOL_IDLE_DISPOSE_MS }

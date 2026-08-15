import type { NotificationPort } from "../ports"

/**
 * Module-level holder for the injected {@link NotificationPort}.
 *
 * Core services (e.g. the harness transport) surface out-of-band errors through
 * this seam; shells install a toast-backed adapter, tests can stub it.
 */

const consoleNotifier: NotificationPort = {
  error(message) {
    console.error(message)
  },
}

let current: NotificationPort = consoleNotifier

/** Install the shell's notification adapter. Call once at app assembly. */
export function setNotifier(port: NotificationPort): void {
  current = port
}

/** The currently installed notification adapter (console by default). */
export function getNotifier(): NotificationPort {
  return current
}

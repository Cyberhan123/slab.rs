import { toast } from "sonner"

import {
  createTauriPorts,
  setImageSrcPort,
  setNotifier,
  tauriImageSrc,
} from "@slab/core"
import type { NotificationPort } from "@slab/core"

/**
 * Desktop (Tauri) platform assembly.
 *
 * Wires the module-level seams in `@slab/core` (image resolution + error
 * notification) once at app startup and exposes the full port set for the
 * React DI context (installed by `SlabProvider` in Phase 2).
 */
const sonnerNotifier: NotificationPort = {
  error(message, options) {
    toast.error(message, {
      description: options?.description,
      id: options?.id,
    })
  },
}

let assembled = false

export function assembleDesktopPlatform() {
  if (assembled) return
  assembled = true
  setImageSrcPort(tauriImageSrc)
  setNotifier(sonnerNotifier)
}

export function createDesktopPorts() {
  assembleDesktopPlatform()
  return createTauriPorts({ notifications: sonnerNotifier })
}

import { toast } from "sonner"

import {
  createTauriPorts,
  setImageSrcPort,
  setNotifier,
  setPluginHost,
  tauriImageSrc,
} from "@slab/core"
import { tauriPluginHost } from "@slab/core/infra/tauri/plugin-host-bridge"
import type { NotificationPort } from "@slab/core"

/**
 * Desktop (Tauri) platform assembly.
 *
 * Wires the module-level seams in `@slab/core` (image resolution + error
 * notification) once at app startup and exposes the full port set for the
 * React DI context (installed by `SlabProvider` in the shell's `main.tsx`).
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
  setPluginHost(tauriPluginHost)
}

export function createDesktopPorts() {
  assembleDesktopPlatform()
  return createTauriPorts({ notifications: sonnerNotifier })
}

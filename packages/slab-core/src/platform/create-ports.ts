import { tauriFileDialog } from "../infra/tauri/file-dialog-adapter"
import { tauriImageSrc } from "../infra/tauri/image-src-adapter"
import { tauriMediaFile } from "../infra/tauri/media-file-adapter"
import { webFileDialog } from "../infra/web/file-dialog-adapter"
import { webImageSrc } from "../infra/web/image-src-adapter"
import { webMediaFile } from "../infra/web/media-file-adapter"
import { detectPlatformInfo, isTauri } from "./detect"
import type {
  NotificationPort,
  SlabPorts,
} from "../ports"

/** Tauri adapters degrade to web behavior outside the desktop shell. */
const desktopFileDialog = isTauri() ? tauriFileDialog : webFileDialog
const desktopMediaFile = isTauri() ? tauriMediaFile : webMediaFile
const desktopImageSrc = isTauri() ? tauriImageSrc : webImageSrc

/**
 * Platform capability set for the desktop (Tauri) shell. `notifications` is
 * injected by the shell (toast presentation lives with the UI layer).
 */
export function createTauriPorts(options?: {
  notifications?: NotificationPort
}): SlabPorts {
  return {
    fileDialog: desktopFileDialog,
    mediaFile: desktopMediaFile,
    imageSrc: desktopImageSrc,
    notifications:
      options?.notifications ?? {
        error(message) {
          console.error(message)
        },
      },
    platformInfo: detectPlatformInfo(),
  }
}

/** Platform capability set for the plain-web shell. */
export function createWebPorts(options?: {
  notifications?: NotificationPort
}): SlabPorts {
  return {
    fileDialog: webFileDialog,
    mediaFile: webMediaFile,
    imageSrc: webImageSrc,
    notifications:
      options?.notifications ?? {
        error(message) {
          console.error(message)
        },
      },
    platformInfo: detectPlatformInfo(),
  }
}

/**
 * Platform capability set for the mobile H5 shell. Currently identical to the
 * web set; mobile-specific overrides (e.g. capture-backed file picking) land
 * here.
 */
export function createH5Ports(options?: {
  notifications?: NotificationPort
}): SlabPorts {
  return createWebPorts(options)
}

// Re-export the adapters shells need for the module-level seams.
export { tauriImageSrc, webImageSrc }

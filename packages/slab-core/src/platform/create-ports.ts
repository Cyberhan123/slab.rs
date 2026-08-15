import { h5FileDialog } from "../infra/h5/file-dialog-adapter"
import { tauriFileDialog } from "../infra/tauri/file-dialog-adapter"
import { tauriImageSrc } from "../infra/tauri/image-src-adapter"
import { tauriMediaFile } from "../infra/tauri/media-file-adapter"
import { tauriWindowChrome } from "../infra/tauri/window-chrome-adapter"
import { webFileDialog } from "../infra/web/file-dialog-adapter"
import { webImageSrc } from "../infra/web/image-src-adapter"
import { webMediaFile } from "../infra/web/media-file-adapter"
import { webWindowChrome } from "../infra/web/window-chrome-adapter"
import { detectPlatformInfo, isTauri } from "./detect"
import type {
  NotificationPort,
  SlabPorts,
  WindowChromePort,
} from "../ports"

/** Tauri adapters degrade to web behavior outside the desktop shell. */
const desktopFileDialog = isTauri() ? tauriFileDialog : webFileDialog
const desktopMediaFile = isTauri() ? tauriMediaFile : webMediaFile
const desktopImageSrc = isTauri() ? tauriImageSrc : webImageSrc
const desktopWindowChrome: WindowChromePort = isTauri()
  ? tauriWindowChrome
  : webWindowChrome

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
    windowChrome: desktopWindowChrome,
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
    windowChrome: webWindowChrome,
  }
}

/**
 * Platform capability set for the mobile H5 shell: the web set with an
 * input-backed file dialog (the mobile browser's own picker/camera flow).
 */
export function createH5Ports(options?: {
  notifications?: NotificationPort
}): SlabPorts {
  return {
    ...createWebPorts(options),
    fileDialog: h5FileDialog,
  }
}

// Re-export the adapters shells need for the module-level seams.
export { tauriImageSrc, webImageSrc }

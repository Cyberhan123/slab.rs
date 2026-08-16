import { getCurrentWindow } from "@tauri-apps/api/window"

import { isTauri } from "../../platform/detect"
import type { WindowChromePort } from "../../ports"

/** Native window controls via the Tauri window API. */
export const tauriWindowChrome: WindowChromePort = {
  async minimize() {
    await getCurrentWindow().minimize()
  },
  async toggleMaximize() {
    await getCurrentWindow().toggleMaximize()
  },
  async close() {
    await getCurrentWindow().close()
  },
  isAvailable() {
    return isTauri()
  },
}

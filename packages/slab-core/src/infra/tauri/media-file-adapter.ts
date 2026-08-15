import { convertFileSrc } from "@tauri-apps/api/core"

import { isTauri } from "../../platform/detect"
import type { MediaFilePort } from "../../ports"

/**
 * Read a local (native-path) file as a Blob via the Tauri asset protocol.
 * Outside the Tauri shell this rejects — callers should use web `File`
 * objects instead.
 */
export const tauriMediaFile: MediaFilePort = {
  async readFile(path) {
    if (!isTauri()) {
      throw new Error("local file reads are only available in Tauri mode")
    }
    const response = await fetch(convertFileSrc(path))
    if (!response.ok) {
      throw new Error(`failed to read local file '${path}': ${response.status}`)
    }
    return response.blob()
  },
}

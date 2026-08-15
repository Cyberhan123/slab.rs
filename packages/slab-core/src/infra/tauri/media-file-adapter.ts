import { convertFileSrc, invoke } from "@tauri-apps/api/core"

import { isTauri } from "../../platform/detect"
import type { MediaFilePort } from "../../ports"

function tauriOnly(action: string): never {
  throw new Error(`${action} is only available in Tauri mode`)
}

/**
 * Local media access on the desktop shell: reads via the Tauri asset
 * protocol and stages recorder bytes through the host's temp-file commands.
 * Outside the Tauri shell every method rejects — callers should use web
 * `File` objects instead.
 */
export const tauriMediaFile: MediaFilePort = {
  async readFile(path) {
    if (!isTauri()) {
      tauriOnly("local file reads")
    }
    const response = await fetch(convertFileSrc(path))
    if (!response.ok) {
      throw new Error(`failed to read local file '${path}': ${response.status}`)
    }
    return response.blob()
  },

  async writeTempAudio(bytes, extension) {
    if (!isTauri()) {
      tauriOnly("temp audio staging")
    }
    return invoke<string>("write_temp_audio", {
      bytes: Array.from(bytes),
      extension,
    })
  },

  async removeTempAudio(path) {
    if (!isTauri()) return
    await invoke("remove_temp_audio", { path })
  },
}

import { isTauri } from "../../platform/detect"
import type { FileDialogPort, PickedFile } from "../../ports"

/**
 * Tauri-backed native file/folder dialogs.
 *
 * Outside the Tauri shell (e.g. `vite dev` in a plain browser) every method
 * degrades to the browser behavior so callers can fall back to a manual path
 * input or an `<input type="file">` element.
 */
export const tauriFileDialog: FileDialogPort = {
  async pickFolder() {
    if (!isTauri()) return null
    const { open } = await import("@tauri-apps/plugin-dialog")
    const selected = await open({ directory: true, multiple: false })
    return typeof selected === "string" ? selected : null
  },

  async pickFile(options): Promise<PickedFile | null> {
    const picked = await this.pickFiles({ ...options, multiple: false })
    return picked[0] ?? null
  },

  async pickFiles(options): Promise<PickedFile[]> {
    if (!isTauri()) return []
    const { open } = await import("@tauri-apps/plugin-dialog")
    const selected = await open({
      multiple: options?.multiple ?? true,
      filters: options?.filters,
    })
    const list = Array.isArray(selected) ? selected : selected === null ? [] : [selected]
    return list
      .filter((entry): entry is string => typeof entry === "string")
      .map((path) => ({ path, name: path.split(/[/\\]/).pop() ?? path }))
  },
}

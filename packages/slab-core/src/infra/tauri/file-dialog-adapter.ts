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
    if (!isTauri()) return null
    const { open } = await import("@tauri-apps/plugin-dialog")
    const selected = await open({
      multiple: options?.multiple ?? false,
      filters: options?.filters,
    })
    if (selected === null) return null
    const first = Array.isArray(selected) ? selected[0] : selected
    if (typeof first !== "string") return null
    return { path: first, name: first.split(/[/\\]/).pop() ?? first }
  },
}

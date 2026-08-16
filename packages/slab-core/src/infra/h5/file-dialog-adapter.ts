import type { FileDialogPort, PickedFile, PickFileOptions } from "../../ports"

/** Build an `accept` value from picker filters: `.ext,.ext` (no filter → none). */
function buildAccept(options?: PickFileOptions): string | undefined {
  const extensions = options?.filters?.flatMap((filter) => filter.extensions) ?? []
  if (extensions.length === 0) return undefined
  return extensions.map((extension) => `.${extension.replace(/^\./, "")}`).join(",")
}

/**
 * Spawn a one-shot hidden `<input type="file">`, click it, and resolve with
 * the picked files. Resolves `null`/`[]` when the user cancels — the `cancel`
 * event (supported on modern mobile browsers) plus the fallback `change`
 * without files keep the promise from hanging on older WebViews.
 */
function pickViaInput(options?: PickFileOptions): Promise<File[]> {
  return new Promise((resolve) => {
    const input = document.createElement("input")
    input.type = "file"
    input.hidden = true
    const accept = buildAccept(options)
    if (accept) input.accept = accept
    if (options?.multiple) input.multiple = true

    let settled = false
    const finish = (files: File[]) => {
      if (settled) return
      settled = true
      input.removeEventListener("change", onChange)
      input.removeEventListener("cancel", onCancel)
      input.remove()
      resolve(files)
    }
    const onChange = () => finish(input.files ? Array.from(input.files) : [])
    const onCancel = () => finish([])

    input.addEventListener("change", onChange)
    input.addEventListener("cancel", onCancel)
    document.body.append(input)
    input.click()
  })
}

function toPickedFile(file: File): PickedFile {
  // Web `File` objects carry no native path; `path` stays unset.
  return { file, name: file.name }
}

/**
 * Mobile H5 file picking: no native dialog exists, so picks go through a
 * dynamically spawned `<input type="file">` (the mobile browser offers its
 * own picker, camera, etc.). `pickFolder` still returns `null` — mobile
 * browsers have no directory concept; callers fall back to path entry.
 */
export const h5FileDialog: FileDialogPort = {
  async pickFolder() {
    return null
  },
  async pickFile(options?: PickFileOptions): Promise<PickedFile | null> {
    const files = await pickViaInput(options)
    return files[0] ? toPickedFile(files[0]) : null
  },
  async pickFiles(options?: PickFileOptions): Promise<PickedFile[]> {
    const files = await pickViaInput(options)
    return files.map(toPickedFile)
  },
}

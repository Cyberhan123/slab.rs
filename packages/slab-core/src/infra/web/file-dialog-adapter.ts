import type { FileDialogPort, PickedFile } from "../../ports"

/**
 * Browser-first dialogs: no native folder picker exists, so `pickFolder`
 * returns `null` (callers fall back to a manual path input) and `pickFile`
 * surfaces the change event's `File` object when the caller wires an
 * `<input type="file">` element.
 */
export const webFileDialog: FileDialogPort = {
  async pickFolder() {
    return null
  },
  async pickFile(): Promise<PickedFile | null> {
    // Without a native dialog the caller owns the HTML input; there is
    // nothing to pick proactively.
    return null
  },
  async pickFiles(): Promise<PickedFile[]> {
    return []
  },
}

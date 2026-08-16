import { convertFileSrc } from "@tauri-apps/api/core"

import { isTauri } from "../../platform/detect"
import type { ImageSrcPort } from "../../ports"

/** Tauri asset-protocol adapter: local paths become `asset://` URLs. */
export const tauriImageSrc: ImageSrcPort = {
  resolve(pathOrUrl) {
    return convertFileSrc(pathOrUrl)
  },
  canResolveLocalPaths() {
    return isTauri()
  },
}

import { SERVER_BASE_URL } from "@slab/api/config"

import type { MediaFilePort } from "../../ports"

/**
 * Web media reads: only server-exposed paths (`/v1/...`) are fetchable; bare
 * local paths reject (the browser never had the file — it arrives as a `File`
 * object from an input element instead).
 */
export const webMediaFile: MediaFilePort = {
  async readFile(path) {
    if (!path.startsWith("/v1/")) {
      throw new Error(`cannot read local file '${path}' on the web`)
    }
    const response = await fetch(`${SERVER_BASE_URL}${path}`)
    if (!response.ok) {
      throw new Error(`failed to read '${path}': ${response.status}`)
    }
    return response.blob()
  },
}

import type { ImageSrcPort } from "../../ports"

/**
 * Web image source: URLs and data URLs pass through untouched; local
 * filesystem paths cannot be fetched from a browser.
 */
export const webImageSrc: ImageSrcPort = {
  resolve(pathOrUrl) {
    return pathOrUrl
  },
  canResolveLocalPaths() {
    return false
  },
}

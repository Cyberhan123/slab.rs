import type { ImageSrcPort } from "../ports"

/**
 * Module-level holder for the injected {@link ImageSrcPort}.
 *
 * Mirrors the `@slab/api` client-singleton style: core code that needs image
 * resolution deep in a call graph (e.g. the harness `TurnItem` projector) reads
 * the port from here instead of threading it through every function signature.
 * Shells install their concrete adapter once at assembly time.
 */

const webImageSrc: ImageSrcPort = {
  resolve(pathOrUrl) {
    return pathOrUrl
  },
  canResolveLocalPaths() {
    return false
  },
}

let current: ImageSrcPort = webImageSrc

/** Install the shell's image-source adapter. Call once at app assembly. */
export function setImageSrcPort(port: ImageSrcPort): void {
  current = port
}

/** The currently installed image-source adapter (web identity by default). */
export function getImageSrcPort(): ImageSrcPort {
  return current
}

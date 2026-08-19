import type { SlabPlatformInfo } from "../ports"

type TauriWindow = Window & {
  __TAURI_INTERNALS__?: unknown
}

/**
 * Pure platform detection (no React — safe for `@slab/core`).
 *
 * This is the single detection seam for the whole frontend: adapters branch on
 * it, UI code should prefer the injected `SlabPlatformInfo` port instead.
 */
export function isTauri(): boolean {
  return (
    typeof window !== "undefined" &&
    Boolean((window as TauriWindow)["__TAURI_INTERNALS__"])
  )
}

/** True when running inside a mobile browser. */
export function isMobileWeb(): boolean {
  if (typeof window === "undefined") return false
  const ua = window.navigator?.userAgent ?? ""
  return /Android|iPhone|iPad|iPod/i.test(ua)
}

export function detectPlatformInfo(): SlabPlatformInfo {
  return {
    desktop: isTauri(),
    mobile: !isTauri() && isMobileWeb(),
  }
}

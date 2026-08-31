import type { SlabPlatformInfo, SlabPlatformOs } from "../ports"

export type { SlabPlatformOs }

type TauriWindow = Window & {
  __TAURI_INTERNALS__?: unknown
}

type NavigatorWithUserAgentData = Navigator & {
  userAgentData?: { platform?: string }
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

/** Best-effort desktop OS family from the modern hint or user-agent parsing. */
export function detectPlatformOs(): SlabPlatformOs {
  if (typeof navigator === "undefined") return "unknown"

  // Prefer the low-entropy client hint when present; `navigator.platform` is
  // deprecated, so the user agent is the only fallback.
  const navigatorWithHints = navigator as NavigatorWithUserAgentData
  const hinted = navigatorWithHints.userAgentData?.platform?.toLowerCase() ?? ""
  const userAgent = navigator.userAgent?.toLowerCase() ?? ""

  if (hinted.includes("mac") || userAgent.includes("mac os")) {
    return "macos"
  }

  if (hinted.includes("win") || userAgent.includes("windows")) {
    return "windows"
  }

  if (hinted.includes("linux") || userAgent.includes("linux")) {
    return "linux"
  }

  return "unknown"
}

export function detectPlatformInfo(): SlabPlatformInfo {
  return {
    desktop: isTauri(),
    mobile: !isTauri() && isMobileWeb(),
    os: detectPlatformOs(),
  }
}

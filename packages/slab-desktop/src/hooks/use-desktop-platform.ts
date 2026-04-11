import { useMemo } from "react"

export type DesktopPlatform = "macos" | "windows" | "linux" | "unknown"

function detectDesktopPlatform(): DesktopPlatform {
  if (typeof navigator === "undefined") {
    return "unknown"
  }

  const platform = navigator.platform.toLowerCase()
  const userAgent = navigator.userAgent.toLowerCase()

  if (platform.includes("mac") || userAgent.includes("mac os")) {
    return "macos"
  }

  if (platform.includes("win") || userAgent.includes("windows")) {
    return "windows"
  }

  if (platform.includes("linux") || userAgent.includes("linux")) {
    return "linux"
  }

  return "unknown"
}

export function getDesktopPlatform(): DesktopPlatform {
  return detectDesktopPlatform()
}

export default function useDesktopPlatform() {
  return useMemo(() => detectDesktopPlatform(), [])
}

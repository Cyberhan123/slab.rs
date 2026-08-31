import { useMemo } from "react"

import { detectPlatformOs, type SlabPlatformOs } from "@slab/core/platform/detect"

export type DesktopPlatform = SlabPlatformOs

/** Delegates to the @slab/core detection seam (single source for the OS). */
export function getDesktopPlatform(): DesktopPlatform {
  return detectPlatformOs()
}

export default function useDesktopPlatform() {
  return useMemo(() => detectPlatformOs(), [])
}

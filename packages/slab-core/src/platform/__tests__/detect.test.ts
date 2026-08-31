import { afterEach, describe, expect, it, vi } from "vitest"

import { detectPlatformInfo, detectPlatformOs } from "../detect"

describe("platform detection seam", () => {
  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it("prefers the userAgentData platform hint", () => {
    vi.stubGlobal("navigator", {
      userAgentData: { platform: "Windows" },
      userAgent: "Mozilla/5.0",
    })

    expect(detectPlatformOs()).toBe("windows")
  })

  it("falls back to user-agent parsing", () => {
    vi.stubGlobal("navigator", {
      userAgent: "Mozilla/5.0 (Macintosh; Mac OS X 10_15_7)",
    })

    expect(detectPlatformOs()).toBe("macos")
  })

  it("returns unknown when no signal matches", () => {
    vi.stubGlobal("navigator", {
      userAgent: "Mozilla/5.0 (X11; FreeBSD amd64)",
    })

    expect(detectPlatformOs()).toBe("unknown")
  })

  it("detectPlatformInfo carries the os dimension", () => {
    vi.stubGlobal("window", {
      navigator: { userAgent: "Mozilla/5.0 (Windows NT 10.0)" },
    })
    vi.stubGlobal("navigator", {
      userAgent: "Mozilla/5.0 (Windows NT 10.0)",
    })

    expect(detectPlatformInfo()).toEqual({
      desktop: false,
      mobile: false,
      os: "windows",
    })
  })
})

import { renderHook } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@slab/i18n", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))

import { useGreeting } from "../use-greeting"

describe("useGreeting", () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it("returns the morning greeting before noon", () => {
    vi.setSystemTime(new Date(2026, 5, 18, 7, 0, 0))
    const { result } = renderHook(() => useGreeting())
    expect(result.current).toBe("pages.assistant.greeting.morning")
  })

  it("returns the afternoon greeting between noon and 18:00", () => {
    vi.setSystemTime(new Date(2026, 5, 18, 13, 0, 0))
    const { result } = renderHook(() => useGreeting())
    expect(result.current).toBe("pages.assistant.greeting.afternoon")
  })

  it("returns the evening greeting at or after 18:00", () => {
    vi.setSystemTime(new Date(2026, 5, 18, 20, 0, 0))
    const { result } = renderHook(() => useGreeting())
    expect(result.current).toBe("pages.assistant.greeting.evening")
  })

  it("honors the 12:00 and 18:00 boundaries", () => {
    vi.setSystemTime(new Date(2026, 5, 18, 0, 0, 0))
    expect(renderHook(() => useGreeting()).result.current).toBe("pages.assistant.greeting.morning")

    vi.setSystemTime(new Date(2026, 5, 18, 11, 59, 59))
    expect(renderHook(() => useGreeting()).result.current).toBe("pages.assistant.greeting.morning")

    vi.setSystemTime(new Date(2026, 5, 18, 12, 0, 0))
    expect(renderHook(() => useGreeting()).result.current).toBe("pages.assistant.greeting.afternoon")

    vi.setSystemTime(new Date(2026, 5, 18, 17, 59, 59))
    expect(renderHook(() => useGreeting()).result.current).toBe("pages.assistant.greeting.afternoon")

    vi.setSystemTime(new Date(2026, 5, 18, 18, 0, 0))
    expect(renderHook(() => useGreeting()).result.current).toBe("pages.assistant.greeting.evening")
  })
})

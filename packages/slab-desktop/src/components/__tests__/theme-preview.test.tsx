import { cleanup, render } from "@testing-library/react"
import { afterEach, describe, expect, it, vi } from "vitest"

import { ThemePreview } from "../theme-preview"

describe("ThemePreview", () => {
  afterEach(() => {
    vi.useRealTimers()
    cleanup()
  })

  // Showcase component: mounts ~30 Radix primitives + an auto-advancing
  // progress interval. Smoke-test only — assert it mounts without throwing.
  it("mounts without throwing and renders content", () => {
    vi.useFakeTimers()

    const { container } = render(<ThemePreview />)

    expect(container.childElementCount).toBeGreaterThan(0)
  })
})

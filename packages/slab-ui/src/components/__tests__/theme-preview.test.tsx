import { render } from "vitest-browser-react"
import { afterEach, describe, expect, it, vi } from "vitest"

import { ThemePreview } from "../theme-preview"

describe("ThemePreview", () => {
  afterEach(() => {
    vi.useRealTimers()
  })

  // Showcase component: mounts ~30 Radix primitives + an auto-advancing
  // progress interval. Smoke-test only — assert it mounts without throwing.
  it("mounts without throwing and renders content", async () => {
    vi.useFakeTimers()

    const { container } = await render(<ThemePreview />)

    expect(container.childElementCount).toBeGreaterThan(0)
  })
})

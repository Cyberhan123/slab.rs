import { beforeEach, describe, expect, it, vi } from "vitest"
import { render } from "vitest-browser-react"
import { Streamdown } from "streamdown"

import { Markdown } from "../message/markdown"

// `Markdown` is a thin wrapper around `Streamdown`. Mock the heavy renderer so
// the test stays focused on the wrapper's prop logic (defaults, className merge,
// streaming-mode override) instead of Streamdown's HTML output.
vi.mock("streamdown", () => ({
  Streamdown: vi.fn<(props: unknown) => unknown>((_props: unknown) => null),
  useIsCodeFenceIncomplete: () => false,
}))

const MockStreamdown = vi.mocked(Streamdown)

function lastProps(): Record<string, unknown> {
  return (MockStreamdown.mock.calls.at(-1)?.[0] ?? {}) as Record<string, unknown>
}

describe("Markdown", () => {
  beforeEach(() => {
    MockStreamdown.mockClear()
  })

  it("delegates rendering to Streamdown and forwards children", async () => {
    await render(<Markdown>{"# hello"}</Markdown>)

    expect(MockStreamdown).toHaveBeenCalledTimes(1)
    expect(lastProps().children).toBe("# hello")
  })

  it("defaults controls to false and tags the slot", async () => {
    await render(<Markdown>{"x"}</Markdown>)

    expect(lastProps().controls).toBe(false)
    expect(lastProps()["data-slot"]).toBe("markdown")
  })

  it("applies the base markdown className and merges a custom one", async () => {
    await render(<Markdown className="prose-sm">{"x"}</Markdown>)

    const className = lastProps().className as string
    expect(className).toContain("cn-markdown")
    expect(className).toContain("w-full")
    expect(className).toContain("min-w-0")
    expect(className).toContain("prose-sm")
  })

  it("provides the default plugin set (cjk, mermaid, math — fences go to CodeBlock)", async () => {
    await render(<Markdown>{"x"}</Markdown>)

    const pluginKeys = Object.keys(lastProps().plugins as object)
    expect(pluginKeys).toHaveLength(3)
    expect(pluginKeys).toEqual(expect.arrayContaining(["cjk", "math", "mermaid"]))
  })

  it("routes fenced code to the CodeBlock override", async () => {
    await render(<Markdown>{"x"}</Markdown>)

    const components = lastProps().components as { code?: unknown }
    expect(typeof components.code).toBe("function")
  })

  it("forces streaming mode while another chunk is pending", async () => {
    await render(<Markdown hasNextChunk>{"x"}</Markdown>)

    expect(lastProps().mode).toBe("streaming")
    // isAnimating drives useIsCodeFenceIncomplete inside the code override.
    expect(lastProps().isAnimating).toBe(true)
  })

  it("forwards an explicit mode when not streaming", async () => {
    await render(<Markdown mode="static">{"x"}</Markdown>)

    expect(lastProps().mode).toBe("static")
  })
})

import type { ReactNode } from "react"
import { describe, expect, it, vi } from "vitest"
import { render } from "vitest-browser-react"

import type { MessagePartRenderProps } from "../message/message-parts"
import type { TMessage, TMessagePart } from "../message/message-item"
import MessageTextPart from "../message/message-text-part"

vi.mock("../message/markdown", () => ({
  Markdown: ({
    children,
    hasNextChunk,
  }: {
    children: ReactNode
    hasNextChunk?: boolean
  }) => (
    <div data-testid="md" data-streaming={hasNextChunk ? "true" : "false"}>
      {children}
    </div>
  ),
}))

type TextPartProps = MessagePartRenderProps<TMessagePart, TMessage>

function baseProps(partOverrides: Partial<TMessagePart> = {}, kind: TextPartProps["kind"] = "text"): TextPartProps {
  const part = { type: "text", text: "hello", ...partOverrides } as TMessagePart
  const message = { id: "m1", role: "assistant" } as TMessage
  return {
    item: { key: "p0", part, message, index: 0, kind },
    part,
    message,
    index: 0,
    kind,
  }
}

describe("MessageTextPart", () => {
  it("renders the markdown content for a text part", async () => {
    const screen = await render(<MessageTextPart {...baseProps()} />)

    const md = screen.getByTestId("md")
    await expect.element(md).toHaveTextContent("hello")
    await expect.element(md).toHaveAttribute("data-streaming", "false")
  })

  it("keeps markdown in streaming mode while the chunk is streaming", async () => {
    const screen = await render(<MessageTextPart {...baseProps({ state: "streaming", text: "partial" })} />)

    await expect.element(screen.getByTestId("md")).toHaveAttribute("data-streaming", "true")
  })

  it("renders nothing for non-text kinds", async () => {
    const screen = await render(<MessageTextPart {...baseProps({}, "tool")} />)

    await expect.element(screen.getByTestId("md")).not.toBeInTheDocument()
  })
})

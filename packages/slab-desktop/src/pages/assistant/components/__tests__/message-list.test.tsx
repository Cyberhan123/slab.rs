import { render, screen } from "@testing-library/react"
import type { ReactNode } from "react"
import { describe, expect, it, vi } from "vitest"

import { setupSlabI18nMock } from "@slab/test-utils/mocks"

import MessageList from "../message/message-list"

vi.mock("@slab/i18n", () => setupSlabI18nMock())

vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: ({
    count,
    getItemKey,
  }: {
    count: number
    getItemKey?: (index: number) => string | number
  }) => ({
    getTotalSize: () => count * 86,
    getVirtualItems: () =>
      Array.from({ length: count }, (_, index) => ({
        key: getItemKey ? getItemKey(index) : index,
        index,
        start: index * 86,
        measureElement: () => {},
      })),
  }),
}))

vi.mock("@slab/components/marker", () => ({
  Marker: ({
    children,
    ...rest
  }: { children: ReactNode } & Record<string, unknown>) => <div {...rest}>{children}</div>,
  MarkerContent: ({ children }: { children: ReactNode }) => <div>{children}</div>,
}))

vi.mock("@slab/components/message-scroller", () => ({
  MessageScroller: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  MessageScrollerViewport: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  MessageScrollerContent: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  MessageScrollerButton: () => <div />,
}))

vi.mock("../message/message-item", () => ({
  MessageItem: ({ message }: { message: { id: string; role: string } }) => (
    <div data-testid={`message-item-${message.id}`}>{message.role}</div>
  ),
}))

const messages = [
  { id: "m1", role: "assistant" },
  { id: "m2", role: "user" },
]

describe("MessageList", () => {
  it("renders one row per message", () => {
    render(<MessageList messages={messages} isBusy={false} />)

    expect(screen.getByTestId("message-item-m1")).toBeInTheDocument()
    expect(screen.getByTestId("message-item-m2")).toBeInTheDocument()
  })

  it("prepends the history-restored marker when requested and there are messages", () => {
    render(<MessageList messages={messages} isBusy={false} showHistoryMarker />)

    expect(screen.getByTestId("assistant-history-marker")).toBeInTheDocument()
  })

  it("omits the marker when showHistoryMarker is false", () => {
    render(<MessageList messages={messages} isBusy={false} />)

    expect(screen.queryByTestId("assistant-history-marker")).not.toBeInTheDocument()
  })

  it("never shows the marker for an empty session even when requested", () => {
    render(<MessageList messages={[]} isBusy={false} showHistoryMarker />)

    expect(screen.queryByTestId("assistant-history-marker")).not.toBeInTheDocument()
  })
})

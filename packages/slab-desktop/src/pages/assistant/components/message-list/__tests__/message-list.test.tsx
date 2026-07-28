import { render, screen } from "@testing-library/react"
import type { ReactNode } from "react"
import { describe, expect, it, vi } from "vitest"

import { setupSlabI18nMock } from "@slab/test-utils/mocks"

import MessageList from "../message-list"

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

vi.mock("@/pages/assistant/components/message/message-item", () => ({
  MessageItem: ({ message }: { message: { id: string; role: string } }) => (
    <div data-testid={`message-item-${message.id}`}>{message.role}</div>
  ),
}))

vi.mock("@/pages/assistant/components/message/shimmer", () => ({
  Shimmer: ({ children }: { children: ReactNode }) => <span data-testid="shimmer">{children}</span>,
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

  it("renders the history-restored marker at the bottom of the restored history", () => {
    render(<MessageList messages={messages} isBusy={false} showHistoryMarker />)

    // `historyCount` defaults to all messages, so the marker lands at the end.
    expect(screen.getByTestId("assistant-history-marker")).toBeInTheDocument()
  })

  it("places the history marker between restored and live messages by historyCount", () => {
    // historyCount=1 → marker after the first (restored) message, before the second (live).
    const { container } = render(
      <MessageList messages={messages} isBusy={false} showHistoryMarker historyCount={1} />,
    )
    const order = Array.from(container.querySelectorAll("[data-testid]")).map((el) =>
      el.getAttribute("data-testid"),
    )
    expect(order.indexOf("assistant-history-marker")).toBeGreaterThan(
      order.indexOf("message-item-m1"),
    )
    expect(order.indexOf("assistant-history-marker")).toBeLessThan(
      order.indexOf("message-item-m2"),
    )
  })

  it("shows a shimmer while a compaction is in progress and plain text when done", () => {
    const compacting = {
      id: "auto:t1:1",
      mode: "auto" as const,
      phase: "compacting" as const,
      threadId: "t1",
    }
    const { rerender } = render(
      <MessageList messages={messages} isBusy={false} compactionMarkers={[compacting]} />,
    )
    expect(screen.getByTestId("assistant-compact-marker-auto:t1:1")).toBeInTheDocument()
    expect(screen.getByTestId("shimmer")).toBeInTheDocument()

    rerender(
      <MessageList
        messages={messages}
        isBusy={false}
        compactionMarkers={[{ ...compacting, phase: "compacted" as const }]}
      />,
    )
    expect(screen.queryByTestId("shimmer")).toBeNull()
    expect(screen.getByTestId("assistant-compact-marker-auto:t1:1")).toBeInTheDocument()
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

import type { ReactNode } from "react"
import { describe, expect, it, vi } from "vitest"
import { render } from "vitest-browser-react"

import { setupSlabI18nMock } from "@slab/test-utils/mocks"

import { CompactMarkerRow, HistoryMarkerRow } from "../row-components"
import {
  HISTORY_MARKER_ID,
  type ScrollerRowOf,
} from "@/pages/assistant/lib/build-scroller-rows"
import type { CompactionMarker } from "@/pages/assistant/hooks/use-harness-conversation"

vi.mock("@slab/i18n", () => setupSlabI18nMock())

vi.mock("@slab/components/marker", () => ({
  Marker: ({
    children,
    ...rest
  }: { children: ReactNode } & Record<string, unknown>) => <div {...rest}>{children}</div>,
  MarkerContent: ({ children }: { children: ReactNode }) => <div>{children}</div>,
}))

vi.mock("@/pages/assistant/components/message/shimmer", () => ({
  Shimmer: ({ children }: { children: ReactNode }) => <span data-testid="shimmer">{children}</span>,
}))

const compactRow = (overrides: Partial<CompactionMarker> = {}): ScrollerRowOf<"compactMarker"> => {
  const marker: CompactionMarker = {
    id: "manual:t1:1",
    mode: "manual",
    phase: "compacted",
    threadId: "t1",
    ...overrides,
  }
  return { kind: "compactMarker", id: marker.id, marker }
}

describe("CompactMarkerRow", () => {
  it("shimmers while compacting in manual mode", async () => {
    const screen = await render(<CompactMarkerRow row={compactRow({ phase: "compacting" })} historyCreatedAt={null} />)

    await expect.element(screen.getByTestId("shimmer")).toBeInTheDocument()
  })

  it("renders plain text when compacted in manual mode (no shimmer)", async () => {
    const screen = await render(<CompactMarkerRow row={compactRow({ phase: "compacted" })} historyCreatedAt={null} />)

    expect(screen.getByTestId("shimmer").query()).toBeNull()
    await expect.element(screen.getByTestId("assistant-compact-marker-manual:t1:1")).toBeInTheDocument()
  })

  it("shimmers while compacting in auto mode", async () => {
    const screen = await render(
      <CompactMarkerRow
        row={compactRow({ id: "auto:t1:1", mode: "auto", phase: "compacting" })}
        historyCreatedAt={null}
      />,
    )

    await expect.element(screen.getByTestId("shimmer")).toBeInTheDocument()
  })

  it("renders plain text when compacted in auto mode", async () => {
    const screen = await render(
      <CompactMarkerRow
        row={compactRow({ id: "auto:t1:1", mode: "auto", phase: "compacted" })}
        historyCreatedAt={null}
      />,
    )

    expect(screen.getByTestId("shimmer").query()).toBeNull()
    await expect.element(screen.getByTestId("assistant-compact-marker-auto:t1:1")).toBeInTheDocument()
  })

  it("stamps the marker id on the separator", async () => {
    const screen = await render(<CompactMarkerRow row={compactRow({ id: "auto:t9:3" })} historyCreatedAt={null} />)

    await expect.element(screen.getByTestId("assistant-compact-marker-auto:t9:3")).toBeInTheDocument()
  })
})

describe("HistoryMarkerRow", () => {
  it("renders the restored separator", async () => {
    const screen = await render(
      <HistoryMarkerRow
        row={{ kind: "historyMarker", id: HISTORY_MARKER_ID }}
        historyCreatedAt={null}
      />,
    )

    await expect.element(screen.getByTestId("assistant-history-marker")).toBeInTheDocument()
  })

  it("formats the provided createdAt as the label", async () => {
    await render(
      <HistoryMarkerRow
        row={{ kind: "historyMarker", id: HISTORY_MARKER_ID }}
        historyCreatedAt={new Date(2026, 0, 5).getTime()}
      />,
    )

    const text = document.body.textContent ?? ""
    expect(text).toContain("2026-01-05")
  })
})

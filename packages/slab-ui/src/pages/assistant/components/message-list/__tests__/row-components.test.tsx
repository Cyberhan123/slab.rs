import type { ReactNode } from "react"
import { describe, expect, it, vi } from "vitest"
import { render } from "vitest-browser-react"


import { CompactMarkerRow, HistoryMarkerRow, ModelLoadMarkerRow, QueuedInputRow, SessionLoadMarkerRow, SettingsMarkerRow } from "../row-components"
import {
  HISTORY_MARKER_ID,
  MODEL_LOAD_MARKER_ID,
  SESSION_LOAD_MARKER_ID,
  type ScrollerRowOf,
} from "@slab/ui/pages/assistant/lib/build-scroller-rows"
import type { CompactionMarker } from "@slab/core/harness"

vi.mock("@slab/i18n", async () => {
  const { setupSlabI18nMock } = await import("@slab/test-utils/mocks")
  // The default passthrough `t` drops interpolation values; the settings
  // marker labels assert their from/to values, so append them after the key
  // (existing key-only `toContain` assertions still pass through).
  return setupSlabI18nMock({
    useTranslation: () => ({
      t: (key: string, values?: Record<string, unknown>) =>
        values ? `${key} ${Object.values(values).join(" ")}` : key,
      i18n: { resolvedLanguage: "en-US", language: "en-US" },
    }),
  })
})

vi.mock("@slab/components/marker", () => ({
  Marker: ({
    children,
    ...rest
  }: { children: ReactNode } & Record<string, unknown>) => <div {...rest}>{children}</div>,
  MarkerContent: ({ children }: { children: ReactNode }) => <div>{children}</div>,
}))

vi.mock("@slab/components/message", () => ({
  Message: ({
    children,
    ...rest
  }: { children: ReactNode } & Record<string, unknown>) => <div {...rest}>{children}</div>,
  MessageAvatar: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  MessageContent: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  MessageHeader: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  MessageFooter: ({ children }: { children: ReactNode }) => <div>{children}</div>,
}))

vi.mock("@slab/components/bubble", () => ({
  Bubble: ({
    children,
    ...rest
  }: { children: ReactNode } & Record<string, unknown>) => <div {...rest}>{children}</div>,
  BubbleContent: ({ children }: { children: ReactNode }) => <div>{children}</div>,
}))

vi.mock("@slab/ui/pages/assistant/components/user-avatar", () => ({
  default: ({ name }: { name: string }) => <div data-testid="queued-user-avatar">{name}</div>,
}))

vi.mock("@slab/ui/pages/assistant/components/message/shimmer", () => ({
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

describe("SessionLoadMarkerRow", () => {
  it("renders the loading title in a separator marker", async () => {
    const screen = await render(
      <SessionLoadMarkerRow
        row={{ kind: "sessionLoadMarker", id: SESSION_LOAD_MARKER_ID }}
        historyCreatedAt={null}
      />,
    )

    await expect.element(screen.getByTestId("assistant-session-load-marker")).toBeInTheDocument()
    expect(screen.getByTestId("shimmer").element().textContent).toContain("pages.assistant.loading.title")
  })
})

describe("ModelLoadMarkerRow", () => {
  it("renders the downloading label and percent at the live edge", async () => {
    const screen = await render(
      <ModelLoadMarkerRow
        row={{
          kind: "modelLoadMarker",
          id: MODEL_LOAD_MARKER_ID,
          modelLoad: { phase: "downloading", downloadedBytes: 25, totalBytes: 100 },
        }}
        historyCreatedAt={null}
      />,
    )

    await expect.element(screen.getByTestId("assistant-model-load-marker")).toBeInTheDocument()
    expect(screen.getByTestId("shimmer").element().textContent).toContain("pages.assistant.modelLoad.downloading")
    expect(screen.getByText("25%").element()).toBeInTheDocument()
  })

  it("omits the percent when totals are unknown", async () => {
    const screen = await render(
      <ModelLoadMarkerRow
        row={{
          kind: "modelLoadMarker",
          id: MODEL_LOAD_MARKER_ID,
          modelLoad: { phase: "loading" },
        }}
        historyCreatedAt={null}
      />,
    )

    expect(screen.getByText("%").query()).toBeNull()
    expect(screen.getByTestId("shimmer").element().textContent).toContain("pages.assistant.modelLoad.loading")
  })
})

describe("SettingsMarkerRow", () => {
  it("renders the model-switch separator keyed to the marker id", async () => {
    const screen = await render(
      <SettingsMarkerRow
        row={{
          kind: "settingsMarker",
          id: "modelSwitch:1",
          marker: { id: "modelSwitch:1", kind: "modelSwitch", fromModel: "Model A", toModel: "Model B" },
        }}
        historyCreatedAt={null}
      />,
    )

    // The i18n mock passes keys through and appends interpolation values.
    await expect.element(screen.getByTestId("assistant-settings-marker-modelSwitch:1")).toBeInTheDocument()
    expect(screen.getByTestId("assistant-settings-marker-modelSwitch:1").element().textContent).toContain(
      "pages.assistant.settingsMarker.modelSwitched",
    )
    expect(screen.getByTestId("assistant-settings-marker-modelSwitch:1").element().textContent).toContain(
      "Model A",
    )
    expect(screen.getByTestId("assistant-settings-marker-modelSwitch:1").element().textContent).toContain(
      "Model B",
    )
  })

  it("renders the permission-mode label with localized from/to modes", async () => {
    const screen = await render(
      <SettingsMarkerRow
        row={{
          kind: "settingsMarker",
          id: "permissionMode:2",
          marker: {
            id: "permissionMode:2",
            kind: "permissionMode",
            fromMode: "approve_for_me",
            toMode: "request_approval",
          },
        }}
        historyCreatedAt={null}
      />,
    )

    const text = screen.getByTestId("assistant-settings-marker-permissionMode:2").element().textContent ?? ""
    expect(text).toContain("pages.assistant.settingsMarker.permissionModeChanged")
    expect(text).toContain("pages.assistant.composer.permission.approveForMe")
    expect(text).toContain("pages.assistant.composer.permission.requestApproval")
  })

  it("renders the approval-reviewer label for a model change and the prompt label otherwise", async () => {
    const modelChange = await render(
      <SettingsMarkerRow
        row={{
          kind: "settingsMarker",
          id: "approvalReview:3",
          marker: {
            id: "approvalReview:3",
            kind: "approvalReview",
            fromModel: null,
            toModel: "Fast Model",
            promptChanged: true,
          },
        }}
        historyCreatedAt={null}
      />,
    )
    const modelText =
      modelChange.getByTestId("assistant-settings-marker-approvalReview:3").element().textContent ?? ""
    // A model change wins over a prompt change (the model is the actionable fact);
    // an unconfigured "from" renders through the not-set label key.
    expect(modelText).toContain("pages.assistant.settingsMarker.approvalReviewerChanged")
    expect(modelText).toContain("pages.assistant.settingsMarker.approvalReviewerNone")

    const promptOnly = await render(
      <SettingsMarkerRow
        row={{
          kind: "settingsMarker",
          id: "approvalReview:4",
          marker: {
            id: "approvalReview:4",
            kind: "approvalReview",
            fromModel: "Fast Model",
            toModel: "Fast Model",
            promptChanged: true,
          },
        }}
        historyCreatedAt={null}
      />,
    )
    expect(
      promptOnly.getByTestId("assistant-settings-marker-approvalReview:4").element().textContent,
    ).toContain("pages.assistant.settingsMarker.approvalReviewerPromptUpdated")
  })
})

describe("QueuedInputRow", () => {
  it("renders the queued text with the queued label", async () => {
    const screen = await render(
      <QueuedInputRow
        row={{ kind: "queuedInput", id: "__queued_input_0", text: "also check the tests" }}
        historyCreatedAt={null}
      />,
    )

    await expect.element(screen.getByTestId("assistant-queued-input")).toBeInTheDocument()
    expect(screen.getByTestId("assistant-queued-input").element().textContent).toContain(
      "also check the tests",
    )
    expect(screen.getByTestId("assistant-queued-input").element().textContent).toContain(
      "pages.assistant.message.queuedLabel",
    )
  })
})

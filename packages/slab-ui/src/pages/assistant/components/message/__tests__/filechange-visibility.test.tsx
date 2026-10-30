import { describe, expect, it, vi } from "vitest"
import { render } from "vitest-browser-react"
import {
  MessageScroller,
  MessageScrollerContent,
  MessageScrollerProvider,
  MessageScrollerViewport,
} from "@slab/components/message-scroller"

import { MessageItem, type TMessage } from "../message-item"
import {
  MessageInteractionContext,
  type MessageInteractionValue,
} from "../../message-interaction-context"

// Visibility regression for the apply_patch e2e: a fileChange row that
// pended on approval must still show its diff card once the approval
// resolves and the output arrives. The card lives inside the tool row's
// collapsible, so losing the forced-open state hides it from the e2e's
// visibility probe even though the markup is present.

vi.mock("@slab/i18n", async () => {
  const { setupSlabI18nMock } = await import("@slab/test-utils/mocks")
  return setupSlabI18nMock()
})
vi.mock("motion/react", () => ({
  motion: { create: <C,>(component: C): C => component },
  useReducedMotion: () => null,
}))
vi.mock("@mantine/hooks", () => ({
  useClipboard: () => ({ copy: vi.fn<() => void>(), copied: false }),
}))

/** Module-level so the Provider value identity is stable across rerenders. */
const INTERACTION: Record<"pending" | "approved", MessageInteractionValue> = {
  pending: {
    approvalStatusByItemId: new Map([["call-1", "pending"]]),
  } as unknown as MessageInteractionValue,
  approved: {
    approvalStatusByItemId: new Map([["call-1", "approved"]]),
  } as unknown as MessageInteractionValue,
}

const message = (overrides: Partial<Record<string, unknown>>): TMessage => ({
  id: "ma1",
  role: "assistant",
  parts: [
    {
      type: "tool-fileChange",
      toolCallId: "call-1",
      toolName: "fileChange",
      ...overrides,
    },
  ],
})

const FINAL = {
  state: "output-available",
  input: {
    changes: [
      {
        type: "edit",
        path: "happy/target.txt",
        diff: "*** Begin Patch\n*** Update File: happy/target.txt\n-alpha\n+alpha PATCHED\n*** End Patch",
      },
    ],
  },
  output: JSON.stringify({ success: true }),
}

/** The full mounted tree — rerender must receive the same shape. */
const tree = (msg: TMessage, status: "pending" | "approved") => (
  <MessageInteractionContext.Provider value={INTERACTION[status]}>
    <MessageScrollerProvider autoScroll>
      <MessageScroller>
        <MessageScrollerViewport>
          <MessageScrollerContent>
            <MessageItem message={msg} />
          </MessageScrollerContent>
        </MessageScrollerViewport>
      </MessageScroller>
    </MessageScrollerProvider>
  </MessageInteractionContext.Provider>
)

describe("fileChange card visibility around the approval lifecycle", () => {
  it("keeps the finalized card absent in the steady restored-history state", async () => {
    // No approval ever pended on this mount (restored history): completed
    // rows sit collapsed by design — the compact line is the feature and
    // the closed collapsible keeps its content unmounted.
    const screen = await render(tree(message(FINAL), "approved"))

    const card = screen.getByTestId("assistant-tool-file-change")
    await expect.element(card).not.toBeInTheDocument()
  })

  it("keeps the card mounted and visible across pending -> approved -> output", async () => {
    // Mirror the live turn: the row mounts approval-pending (forced open,
    // the patch input already carries the changes), the approval resolves,
    // then the finalized output arrives.
    const screen = await render(
      tree(message({ state: "input-available", input: FINAL.input }), "pending")
    )

    // Pending phase: row forced open (the banner holds the decision).
    const pending = screen.getByTestId("assistant-tool-file-change")
    await expect.element(pending).toBeVisible()

    // Approval resolved + output arrived — same mount, rerendered props.
    await screen.rerender(tree(message(FINAL), "approved"))

    const card = screen.getByTestId("assistant-tool-file-change")
    await expect.element(card).toBeVisible()
    await expect.element(card).toMatchTextContent("-alpha")
  })
})

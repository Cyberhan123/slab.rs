import type { ReactNode } from "react"
import { describe, expect, it, vi } from "vitest"
import { render } from "vitest-browser-react"

import { MessageInteractionContext } from "../../message-interaction-context"
import type { ToolPartLike } from "../message-tool-part"
import MessageToolFileChangePart from "../message-tool-file-change-part"

// Stub the compact-row shell so the real tool logic (deriveState/isToolActive)
// runs without Radix collapsible in jsdom; the diff body assertions target the
// content children the row renders.
vi.mock("../message-tool-row", () => ({
  ToolRow: ({ children }: { children?: ReactNode }) => <div data-testid="tool-row">{children}</div>,
  ToolRowTrigger: () => null,
  ToolRowContent: ({ children }: { children?: ReactNode }) => <div>{children}</div>,
  toolRowIcon: () => null,
}))

vi.mock("../../patch-diff-view", () => ({
  PatchDiffView: ({ diff }: { diff: string }) => (
    <pre data-testid="patch-diff-view">{diff}</pre>
  ),
}))

/** Fresh empty interaction fixtures — the component only reads the maps. */
function emptyInteraction() {
  return {
    approvalStatusByItemId: new Map(),
    userMessageTurnIndex: new Map(),
    rollbackToMessage: undefined,
  }
}

async function renderPart(part: Partial<ToolPartLike>, toolCallId = "call-1") {
  return render(
    <MessageInteractionContext.Provider value={emptyInteraction()}>
      <MessageToolFileChangePart
        part={part as ToolPartLike}
        item={{} as never}
        message={{} as never}
        index={0}
        kind="tool"
        toolCallId={toolCallId}
      />
    </MessageInteractionContext.Provider>,
  )
}

describe("MessageToolFileChangePart", () => {
  it("renders the per-file change list with colored diff previews", async () => {
    const screen = await renderPart({
      type: "tool-output-available",
      toolName: "fileChange",
      input: {
        changes: [
          { path: "new.txt", type: "add", diff: "+hello" },
          { path: "gone.txt", type: "delete" },
        ],
      },
      state: "output-available",
    })
    const card = screen.getByTestId("assistant-tool-file-change")
    expect(card.element().textContent).toContain("new.txt")
    expect(card.element().textContent).toContain("gone.txt")
    const diffs = screen.container.querySelectorAll('[data-testid="patch-diff-view"]')
    expect(diffs).toHaveLength(1)
    expect(diffs[0]?.textContent).toContain("+hello")
  })

  it("renders live apply progress lines while the patch is running", async () => {
    const screen = await renderPart({
      type: "tool-input-available",
      toolName: "fileChange",
      input: { changes: [{ path: "a.txt", type: "edit", diff: "-old\n+new" }] },
    })
    // The intended diff is always shown (PatchDiffView), even mid-run.
    expect(screen.getByTestId("patch-diff-view").element().textContent).toContain("+new")
  })

  it("renders nothing for non-tool kinds", async () => {
    const screen = await render(
      <MessageInteractionContext.Provider value={emptyInteraction()}>
        <MessageToolFileChangePart
          part={{ type: "text" } as unknown as ToolPartLike}
          item={{} as never}
          message={{} as never}
          index={0}
          kind="text"
          toolCallId="call-1"
        />
      </MessageInteractionContext.Provider>,
    )
    expect(screen.container.textContent).toBe("")
  })
})

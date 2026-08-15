import type { ReactNode } from "react"
import { describe, expect, it, vi } from "vitest"
import { render } from "vitest-browser-react"

import { MessageInteractionContext } from "../../message-interaction-context"
import type { ToolPartLike } from "../message-tool-part"
import MessageToolPlanPart from "../message-tool-plan-part"

// Stub the heavy leaf deps so the real tool-card logic (deriveState/isToolActive)
// runs without pulling Radix collapsible into jsdom.
vi.mock("@slab/components/collapsible", () => ({
  Collapsible: ({ children }: { children: ReactNode }) => <div data-testid="collapsible">{children}</div>,
  CollapsibleContent: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  CollapsibleTrigger: ({ children }: { children: ReactNode }) => <div>{children}</div>,
}))

vi.mock("@slab/components/badge", () => ({
  Badge: ({ children }: { children: ReactNode }) => <span data-testid="badge">{children}</span>,
}))

async function renderPart(part: Partial<ToolPartLike>, toolCallId = "call-1") {
  return render(
    <MessageInteractionContext.Provider
      value={{
        approvalStatusByItemId: new Map(),
        liveOutputByItemId: new Map(),
        livePatchByItemId: new Map(),
        userMessageTurnIndex: new Map(),
        rollbackToMessage: undefined,
      }}
    >
      <MessageToolPlanPart
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

const PLAN = {
  plan_id: "plan-0",
  summary: "Ship the feature",
  items: [
    { step: "inspect", status: "completed" as const },
    { step: "implement", status: "in_progress" as const },
    { step: "verify", status: "pending" as const },
  ],
  counts: { pending: 1, in_progress: 1, completed: 1, blocked: 0 },
  current_step: 1,
}

describe("MessageToolPlanPart", () => {
  it("renders the plan summary, counts, and step list", async () => {
    const screen = await renderPart({ type: "tool-plan", input: PLAN, state: "output-available" })

    expect(screen.getByTestId("assistant-tool-plan")).toBeInTheDocument()
    const body = screen.getByTestId("assistant-plan-body").element().textContent ?? ""
    expect(body).toContain("Ship the feature")
    expect(body).toContain("3 steps")
    expect(body).toContain("1 done")
    expect(body).toContain("inspect")
    expect(body).toContain("implement")
    expect(body).toContain("verify")
  })

  it("shows the Completed badge for a finalized plan", async () => {
    const screen = await renderPart({ type: "tool-plan", input: PLAN, state: "output-available" })
    expect(screen.getByTestId("badge").element().textContent).toContain("Completed")
  })

  it("falls back to a 'plan' title when the summary is absent", async () => {
    const screen = await renderPart({
      type: "tool-plan",
      input: { ...PLAN, summary: undefined },
      state: "output-available",
    })
    // The ToolHeader title is the summary or "plan"; the header renders the title text.
    expect(screen.getByTestId("collapsible").element().textContent).toContain("plan")
  })
})

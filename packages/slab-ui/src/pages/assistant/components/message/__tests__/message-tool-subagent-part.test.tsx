import type { ReactNode } from "react"
import { describe, expect, it, vi } from "vitest"
import { render } from "vitest-browser-react"

import { MessageInteractionContext } from "../../message-interaction-context"
import type { SubagentChildItem, SubagentTaskInfo } from "@slab/core/harness"
import type { ToolPartLike } from "../message-tool-part"
import MessageToolSubagentPart from "../message-tool-subagent-part"

// Stub the heavy leaf deps (Radix collapsible) so the real row logic
// (deriveState / live-status override) runs in the browser test harness.
vi.mock("@slab/components/collapsible", () => ({
  Collapsible: ({
    children,
    open,
  }: {
    children: ReactNode
    open?: boolean
  }) => (
    <div data-testid="collapsible" data-open={open ? "true" : "false"}>
      {children}
    </div>
  ),
  CollapsibleContent: ({ children }: { children?: ReactNode }) => <div>{children}</div>,
  CollapsibleTrigger: ({ children }: { children?: ReactNode }) => <div>{children}</div>,
}))

function expectToolState(screen: { container: HTMLElement }, state: string) {
  expect(screen.container.querySelector(`[data-tool-state="${state}"]`)).not.toBeNull()
}

async function renderPart(
  part: Partial<ToolPartLike>,
  task?: SubagentTaskInfo,
  childItems?: readonly SubagentChildItem[],
) {
  const interactionValue = {
    approvalStatusByItemId: new Map(),
    userMessageTurnIndex: new Map(),
    rollbackToMessage: undefined,
    subagentTasksByTaskId: task ? new Map([[task.taskId, task]]) : new Map(),
    // `BACKGROUND_OUTPUT` carries child_thread_id "c1" — key the relayed
    // activity to that id like the controller state would.
    subagentChildItemsByChildId: childItems ? new Map([["c1", childItems]]) : new Map(),
  }
  return render(
    <MessageInteractionContext.Provider value={interactionValue}>
      <MessageToolSubagentPart
        part={part as ToolPartLike}
        item={{} as never}
        message={{} as never}
        index={0}
        kind="tool"
        name="delegate_subagent"
        toolCallId="call-1"
      />
    </MessageInteractionContext.Provider>,
  )
}

const BACKGROUND_OUTPUT =
  '{"background":true,"task_id":"bg-1","child_thread_id":"c1","status":"running","hint":"delegate"}'

describe("MessageToolSubagentPart", () => {
  it("shows the running state while the background delegation is live", async () => {
    const screen = await renderPart(
      {
        type: "tool-delegate_subagent",
        input: { task: "summarize the repo" },
        output: BACKGROUND_OUTPUT,
        state: "output-available",
      },
      { taskId: "bg-1", status: "running" },
    )
    // The part is finalized but the delegation is live — the live state wins.
    expectToolState(screen, "input-available")
    expect(screen.getByTestId("collapsible").element().textContent).toContain("Agent")
    expect(screen.getByTestId("collapsible").element().textContent).toContain("summarize the repo")
    expect(screen.getByTestId("tool-detail-subagent").element().textContent).toContain(
      "status: running",
    )
  })

  it("shows the completed state with the result summary once terminal", async () => {
    const screen = await renderPart(
      {
        type: "tool-delegate_subagent",
        input: { task: "summarize the repo" },
        output: BACKGROUND_OUTPUT,
        state: "output-available",
      },
      { taskId: "bg-1", status: "completed", resultSummary: "child result" },
    )
    expectToolState(screen, "output-available")
    expect(screen.getByTestId("tool-detail-subagent").element().textContent).toContain(
      "child result",
    )
  })

  it("shows the failed state when the subagent errored", async () => {
    const screen = await renderPart(
      {
        type: "tool-delegate_subagent",
        input: { task: "summarize the repo" },
        output: BACKGROUND_OUTPUT,
        state: "output-available",
      },
      { taskId: "bg-1", status: "failed" },
    )
    expectToolState(screen, "output-error")
    expect(screen.getByTestId("tool-detail-subagent").element().textContent).toContain(
      "status: failed",
    )
  })

  it("degrades to the delegated footnote without live state (history reload)", async () => {
    const screen = await renderPart(
      {
        type: "tool-delegate_subagent",
        input: { task: "summarize the repo" },
        output: BACKGROUND_OUTPUT,
        state: "output-available",
      },
    )
    // No live task state: the part state stands; the body explains where the
    // result lives.
    expectToolState(screen, "output-available")
    expect(screen.getByTestId("tool-detail-subagent").element().textContent).toContain(
      "background delegation",
    )
  })

  it("renders the inline completion text for background=false delegations", async () => {
    const screen = await renderPart(
      {
        type: "tool-delegate_subagent",
        input: { task: "summarize the repo" },
        output:
          '{"child_thread_id":"c1","status":"completed","completion_text":"child result","artifact_refs":[]}',
        state: "output-available",
      },
    )
    expectToolState(screen, "output-available")
    expect(screen.getByTestId("tool-detail-subagent").element().textContent).toContain(
      "child result",
    )
  })

  it("renders the relayed child activity inside the card", async () => {
    const screen = await renderPart(
      {
        type: "tool-delegate_subagent",
        input: { task: "summarize the repo" },
        output: BACKGROUND_OUTPUT,
        state: "output-available",
      },
      { taskId: "bg-1", status: "running" },
      [
        { id: "item-1", phase: "completed", label: "tool: read_file" },
        { id: "item-2", phase: "started", label: "command: rg needle" },
      ],
    )
    const activity = screen.getByTestId("tool-detail-subagent-activity").element()
    expect(activity.textContent).toContain("tool: read_file")
    expect(activity.textContent).toContain("command: rg needle")
  })

  it("omits the activity section when no child events arrived", async () => {
    const screen = await renderPart(
      {
        type: "tool-delegate_subagent",
        input: { task: "summarize the repo" },
        output: BACKGROUND_OUTPUT,
        state: "output-available",
      },
      { taskId: "bg-1", status: "running" },
    )
    expect(screen.container.querySelector('[data-testid="tool-detail-subagent-activity"]')).toBeNull()
  })
})

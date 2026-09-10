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
  name = "delegate_subagent",
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
        name={name}
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

  // ── companion subagent tools (subagent_status / _message / _stop) ────────

  it("subagent_status renders the task snapshot and correlates live state", async () => {
    const screen = await renderPart(
      {
        type: "tool-subagent_status",
        input: { task_id: "bg-1" },
        output:
          '{"task":{"task_id":"bg-1","parent_thread_id":"p1","child_thread_id":"c1","task":"summarize the repo","status":"running","result":null}}',
        state: "output-available",
      },
      { taskId: "bg-1", status: "running" },
      undefined,
      "subagent_status",
    )
    // The snapshot's task_id feeds the live-state lookup.
    expectToolState(screen, "input-available")
    const body = screen.getByTestId("tool-detail-subagent").element().textContent ?? ""
    expect(body).toContain("status: running")
    expect(body).toContain("task: bg-1")
    // Registry queries never show the delegation footnote.
    expect(body).not.toContain("background delegation")
  })

  it("subagent_message reports queued delivery from the output envelope", async () => {
    const screen = await renderPart(
      {
        type: "tool-subagent_message",
        input: { task_id: "bg-1", message: "wrap up" },
        output: '{"queued":true,"position":2}',
        state: "output-available",
      },
      undefined,
      undefined,
      "subagent_message",
    )
    const body = screen.getByTestId("tool-detail-subagent").element().textContent ?? ""
    // task_id rides only in the input for this tool — still surfaced.
    expect(body).toContain("status: queued")
    expect(body).toContain("task: bg-1")
    expect(body).not.toContain("background delegation")
  })

  it("subagent_stop renders the stopped snapshot's status and result", async () => {
    const screen = await renderPart(
      {
        type: "tool-subagent_stop",
        input: { task_id: "bg-1" },
        output:
          '{"stopped":{"task_id":"bg-1","parent_thread_id":"p1","child_thread_id":"c1","task":"summarize the repo","status":"stopped","result":"partial findings"}}',
        state: "output-available",
      },
      undefined,
      undefined,
      "subagent_stop",
    )
    const body = screen.getByTestId("tool-detail-subagent").element().textContent ?? ""
    expect(body).toContain("status: stopped")
    expect(body).toContain("partial findings")
  })

  it("degrades safely on a non-JSON output without fabricating a status", async () => {
    const screen = await renderPart(
      {
        type: "tool-subagent_status",
        input: { task_id: "bg-1" },
        output: "not an envelope",
        state: "output-available",
      },
      undefined,
      undefined,
      "subagent_status",
    )
    const body = screen.getByTestId("tool-detail-subagent").element().textContent ?? ""
    // No fabricated "completed": the status span is omitted entirely, the
    // task id still comes from the input.
    expect(body).not.toContain("status:")
    expect(body).toContain("task: bg-1")
  })
})

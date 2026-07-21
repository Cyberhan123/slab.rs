import { cleanup, render, screen } from "@testing-library/react"
import { afterEach, describe, expect, it, vi } from "vitest"
import type { ReactNode } from "react"

import { MessageInteractionContext } from "../message-interaction-context"
import type { ToolPartLike } from "../message-tool-part"
import MessageToolCommandPart from "../message-tool-command-part"

// Stub the heavy leaf deps so the real tool-card logic (deriveState/isToolActive)
// runs without pulling Radix collapsible / ansi-to-react / Mantine into jsdom.
// The Terminal mock mirrors the composed layout: the parent renders `output`
// into a content surface, while TerminalHeader/TerminalTitle surface the input.
vi.mock("../terminal", () => ({
  Terminal: ({ output, isStreaming, children }: { output: string; isStreaming?: boolean; children?: ReactNode }) => (
    <div data-testid="terminal" data-streaming={isStreaming ? "true" : "false"}>
      {children}
      <pre data-testid="terminal-content">{output}</pre>
    </div>
  ),
  TerminalHeader: ({ children }: { children?: ReactNode }) => (
    <div data-testid="terminal-header">{children}</div>
  ),
  TerminalTitle: ({ children, title }: { children?: ReactNode; title?: string }) => (
    <span data-testid="terminal-title" title={title}>
      {children}
    </span>
  ),
  TerminalStatus: () => null,
  TerminalActions: ({ children }: { children?: ReactNode }) => <>{children}</>,
  TerminalCopyButton: () => <button data-testid="terminal-copy">copy</button>,
  TerminalContent: () => null,
}))

vi.mock("../code-block", () => ({
  CodeBlock: ({ code }: { code: string }) => <pre data-testid="code-block">{code}</pre>,
}))

vi.mock("@slab/components/collapsible", () => ({
  Collapsible: ({ children }: { children: ReactNode }) => <div data-testid="collapsible">{children}</div>,
  CollapsibleContent: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  CollapsibleTrigger: ({ children }: { children: ReactNode }) => <div>{children}</div>,
}))

vi.mock("@slab/components/badge", () => ({
  Badge: ({ children }: { children: ReactNode }) => <span data-testid="badge">{children}</span>,
}))

function renderPart(
  part: Partial<ToolPartLike>,
  ctx: { approval?: string; liveOutput?: string },
  toolCallId = "call-1",
) {
  return render(
    <MessageInteractionContext.Provider
      value={{
        approvalStatusByItemId: ctx.approval ? new Map([["call-1", ctx.approval]]) : new Map(),
        liveOutputByItemId: ctx.liveOutput ? new Map([["call-1", ctx.liveOutput]]) : new Map(),
      }}
    >
      <MessageToolCommandPart
        // The component only reads `part`/`kind`/`toolCallId`; the rest of the
        // render-props contract is stubbed to satisfy the type.
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

describe("MessageToolCommandPart", () => {
  afterEach(() => cleanup())

  it("renders streamed live output while the command is active (pending approval)", () => {
    renderPart(
      { type: "tool-input-available", input: { command: "echo hi", cwd: "/repo" } },
      { approval: "pending", liveOutput: "streaming-bytes" },
    )
    const terminal = screen.getByTestId("terminal")
    expect(terminal.getAttribute("data-streaming")).toBe("true")
    // Command input lives in the header; cwd is exposed via the title attribute.
    expect(screen.getByTestId("terminal-header").textContent).toContain("$ echo hi")
    expect(screen.getByTestId("terminal-title").getAttribute("title")).toBe("/repo")
    // Output (live while active) lives in the content surface, not the header.
    expect(screen.getByTestId("terminal-content").textContent).toContain("streaming-bytes")
    // Badge reflects the approval-requested state.
    expect(screen.getByTestId("badge").textContent).toContain("Awaiting Approval")
  })

  it("renders finalized output once the command completes", () => {
    renderPart(
      {
        type: "tool-output-available",
        input: { command: "echo hi", cwd: "/repo" },
        output: "final-result",
        state: "output-available",
      },
      { approval: "approved" },
    )
    const terminal = screen.getByTestId("terminal")
    expect(terminal.getAttribute("data-streaming")).toBe("false")
    expect(screen.getByTestId("terminal-header").textContent).toContain("$ echo hi")
    expect(screen.getByTestId("terminal-content").textContent).toContain("final-result")
    expect(screen.getByTestId("badge").textContent).toContain("Completed")
  })

  it("renders the error text and Error badge when the command failed", () => {
    renderPart(
      {
        type: "tool-output-error",
        input: { command: "bad-cmd", cwd: "/repo" },
        errorText: "boom: command not found",
        state: "output-error",
      },
      {},
    )
    expect(screen.getByTestId("terminal-content").textContent).toContain("boom: command not found")
    expect(screen.getByTestId("badge").textContent).toContain("Error")
  })

  it("renders the Denied badge when the approval was denied", () => {
    renderPart(
      { type: "tool-input-available", input: { command: "echo hi", cwd: "/repo" } },
      { approval: "denied" },
    )
    expect(screen.getByTestId("badge").textContent).toContain("Denied")
  })

  it("does not crash when toolCallId is empty and renders no approval lookup", () => {
    // Empty toolCallId is falsy → the component skips the approval/output map lookups.
    renderPart(
      { type: "tool-input-available", input: { command: "echo hi", cwd: "/repo" } },
      {},
      "",
    )
    // Still renders the command framing in the header without throwing.
    expect(screen.getByTestId("terminal-header").textContent).toContain("$ echo hi")
  })
})

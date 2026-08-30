import type { ReactNode } from "react"
import { describe, expect, it, vi } from "vitest"
import { render } from "vitest-browser-react"

import {
  LiveToolOutputContext,
  MessageInteractionContext,
} from "../../message-interaction-context"
import type { ToolPartLike } from "../message-tool-part"
import MessageToolCommandPart from "../message-tool-command-part"

// Stub the heavy leaf deps so the real tool-card logic (deriveState/isToolActive)
// runs without pulling Radix collapsible / ansi-to-react / Mantine into jsdom.
// The Terminal mock mirrors the composed layout: the parent renders `output`
// and `stderrText` into a content surface, while TerminalHeader/TerminalTitle
// surface the input.
vi.mock("../terminal", () => ({
  Terminal: ({
    output,
    stderrText,
    isStreaming,
    children,
  }: {
    output: string
    stderrText?: string
    isStreaming?: boolean
    children?: ReactNode
  }) => (
    <div data-testid="terminal" data-streaming={isStreaming ? "true" : "false"}>
      {children}
      <pre data-testid="terminal-content">
        {output}
        {stderrText ? <span data-testid="terminal-stderr">{stderrText}</span> : null}
      </pre>
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

/** Assert the compact row's status symbol reflects the given tool state. */
function expectToolState(screen: { container: HTMLElement }, state: string) {
  expect(screen.container.querySelector(`[data-tool-state="${state}"]`)).not.toBeNull()
}

async function renderPart(
  part: Partial<ToolPartLike>,
  ctx: { approval?: string; liveOutput?: string },
  toolCallId = "call-1",
) {
  return render(
    <LiveToolOutputContext.Provider
      value={{
        liveOutputByItemId: ctx.liveOutput ? new Map([["call-1", ctx.liveOutput]]) : new Map(),
        livePatchByItemId: new Map(),
      }}
    >
      <MessageInteractionContext.Provider
        value={{
          approvalStatusByItemId: ctx.approval ? new Map([["call-1", ctx.approval]]) : new Map(),
          userMessageTurnIndex: new Map(),
          rollbackToMessage: undefined,
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
      </MessageInteractionContext.Provider>
    </LiveToolOutputContext.Provider>,
  )
}

describe("MessageToolCommandPart", () => {
  it("renders streamed live output while the command is active (pending approval)", async () => {
    const screen = await renderPart(
      { type: "tool-input-available", input: { command: "echo hi", cwd: "/repo" } },
      { approval: "pending", liveOutput: "streaming-bytes" },
    )
    const terminal = screen.getByTestId("terminal")
    expect(terminal.element().getAttribute("data-streaming")).toBe("true")
    // Command input lives in the header; cwd is exposed via the title attribute.
    expect(screen.getByTestId("terminal-header").element().textContent).toContain("$ echo hi")
    expect(screen.getByTestId("terminal-title").element().getAttribute("title")).toBe("/repo")
    // Output (live while active) lives in the content surface, not the header.
    expect(screen.getByTestId("terminal-content").element().textContent).toContain("streaming-bytes")
    // The collapsed row summarizes the call as `Bash: echo hi`.
    expect(screen.getByTestId("collapsible").element().textContent).toContain("Bash")
    expect(screen.getByTestId("collapsible").element().textContent).toContain("echo hi")
    // Status symbol reflects the approval-requested state.
    expectToolState(screen, "approval-requested")
  })

  it("renders finalized output once the command completes", async () => {
    const screen = await renderPart(
      {
        type: "tool-output-available",
        input: { command: "echo hi", cwd: "/repo" },
        output: "final-result",
        state: "output-available",
      },
      { approval: "approved" },
    )
    const terminal = screen.getByTestId("terminal")
    expect(terminal.element().getAttribute("data-streaming")).toBe("false")
    expect(screen.getByTestId("terminal-header").element().textContent).toContain("$ echo hi")
    expect(screen.getByTestId("terminal-content").element().textContent).toContain("final-result")
    expectToolState(screen, "output-available")
  })

  it("renders the error text and Error badge when the command failed", async () => {
    const screen = await renderPart(
      {
        type: "tool-output-error",
        input: { command: "bad-cmd", cwd: "/repo" },
        errorText: "boom: command not found",
        state: "output-error",
      },
      {},
    )
    expect(screen.getByTestId("terminal-content").element().textContent).toContain("boom: command not found")
    expectToolState(screen, "output-error")
  })

  it("renders the denied status symbol when the approval was denied", async () => {
    const screen = await renderPart(
      { type: "tool-input-available", input: { command: "echo hi", cwd: "/repo" } },
      { approval: "denied" },
    )
    expect(screen.container.querySelector('[data-tool-state="output-denied"]')).not.toBeNull()
  })

  it("splits a finalized SandboxedOutput JSON into stdout + stderr instead of raw JSON", async () => {
    const screen = await renderPart(
      {
        type: "tool-output-available",
        input: { command: "whoami", cwd: "/repo" },
        output: '{"stdout":"cyberhan\\n","stderr":"","exit_code":0,"timed_out":false}',
        state: "output-available",
      },
      {},
    )
    const content = screen.getByTestId("terminal-content").element().textContent ?? ""
    expect(content).toContain("cyberhan")
    expect(content).not.toContain('"stdout"')
    expect(content).not.toContain("exit_code")
    // Empty stderr renders no stderr span.
    await expect.element(screen.getByTestId("terminal-stderr")).not.toBeInTheDocument()
  })

  it("renders failed-command stderr INSIDE the terminal (no separate block)", async () => {
    const screen = await renderPart(
      {
        type: "tool-output-error",
        input: { command: "cargo build", cwd: "/repo" },
        errorText: '{"stdout":"compiling\\n","stderr":"error[E0308]: mismatch","exit_code":101,"timed_out":false}',
        state: "output-error",
      },
      {},
    )
    const content = screen.getByTestId("terminal-content").element().textContent ?? ""
    expect(content).toContain("compiling")
    expect(content).not.toContain("exit_code")
    // Stderr lives in the same terminal surface as stdout, not a standalone
    // error card.
    expect(screen.getByTestId("terminal-stderr").element().textContent).toContain(
      "error[E0308]: mismatch",
    )
    expect(screen.container.querySelector('[data-testid="assistant-command-stderr"]')).toBeNull()
    expectToolState(screen, "output-error")
  })

  it("keeps plain-text output rendering unchanged (unparseable JSON)", async () => {
    const screen = await renderPart(
      {
        type: "tool-output-available",
        input: { command: "echo hi", cwd: "/repo" },
        output: "plain text result",
        state: "output-available",
      },
      {},
    )
    expect(screen.getByTestId("terminal-content").element().textContent).toContain("plain text result")
    await expect.element(screen.getByTestId("terminal-stderr")).not.toBeInTheDocument()
  })

  it("does not crash when toolCallId is empty and renders no approval lookup", async () => {
    // Empty toolCallId is falsy → the component skips the approval/output map lookups.
    const screen = await renderPart(
      { type: "tool-input-available", input: { command: "echo hi", cwd: "/repo" } },
      {},
      "",
    )
    // Still renders the command framing in the header without throwing.
    expect(screen.getByTestId("terminal-header").element().textContent).toContain("$ echo hi")
  })
})

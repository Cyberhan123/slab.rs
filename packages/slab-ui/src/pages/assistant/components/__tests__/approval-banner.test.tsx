import { describe, expect, it, vi } from "vitest"
import { render } from "vitest-browser-react"
import type { ReactNode } from "react"

import { ApprovalCard } from "../approval-banner"
import type { ApprovalRequest } from "../../hooks/use-harness-conversation"

vi.mock("@slab/i18n", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))

vi.mock("@slab/components/button", () => ({
  Button: ({
    children,
    onClick,
    disabled,
  }: {
    children: ReactNode
    onClick?: () => void
    disabled?: boolean
  }) => (
    <button type="button" onClick={onClick} disabled={disabled}>
      {children}
    </button>
  ),
}))

vi.mock("@slab/components/badge", () => ({
  Badge: ({ children }: { children: ReactNode }) => <span>{children}</span>,
}))

vi.mock("@slab/components/spinner", () => ({
  Spinner: (props: { className?: string }) => <span data-testid="spinner" {...props} />,
}))

function commandApproval(overrides: Partial<ApprovalRequest> = {}): ApprovalRequest {
  return {
    itemId: "call-1",
    threadId: "hthread-1",
    kind: "command",
    command: "echo hi",
    cwd: "/repo",
    status: "pending",
    ...overrides,
  }
}

describe("ApprovalCard", () => {
  it("renders the command in a terminal-style block with cwd framing", async () => {
    await render(<ApprovalCard approval={commandApproval()} onResolve={vi.fn()} />)
    const text = document.body.textContent ?? ""
    expect(text).toContain("$ cd /repo")
    expect(text).toContain("echo hi")
    expect(text).toContain("pages.assistant.approval.command")
  })

  it("renders file-change entries with a type badge and optional diff", async () => {
    await render(
      <ApprovalCard
        approval={
          {
            itemId: "call-2",
            threadId: "hthread-1",
            kind: "fileChange",
            status: "pending",
            changes: [
              { path: "src/a.ts", type: "edit", diff: "-old\n+new" },
              { path: "README.md", type: "add" },
            ],
          } as ApprovalRequest
        }
        onResolve={vi.fn()}
      />,
    )
    const text = document.body.textContent ?? ""
    expect(text).toContain("src/a.ts")
    expect(text).toContain("-old")
    expect(text).toContain("README.md")
    expect(text).toContain("pages.assistant.approval.fileChange")
  })

  it("shows only the server-advertised scopes", async () => {
    const screen = await render(
      <ApprovalCard
        approval={commandApproval({ allowedScopes: ["run_once", "deny"] })}
        onResolve={vi.fn()}
      />,
    )
    const buttons = screen.getByRole("button").elements()
    const labels = buttons.map((b) => b.textContent)
    expect(labels).toEqual(
      expect.arrayContaining([
        "pages.assistant.approval.runOnce",
        "pages.assistant.actions.reject",
      ]),
    )
    expect(labels).not.toContain("pages.assistant.approval.always")
    expect(buttons).toHaveLength(2)
  })

  it("falls back to simple approve/reject when no scopes are advertised", async () => {
    const screen = await render(<ApprovalCard approval={commandApproval()} onResolve={vi.fn()} />)
    const buttons = screen.getByRole("button").elements()
    expect(buttons).toHaveLength(2)
    const labels = buttons.map((b) => b.textContent)
    expect(labels).toContain("pages.assistant.actions.approve")
    expect(labels).toContain("pages.assistant.actions.reject")
  })

  it("resolves with (itemId, approved, scope) when a scope button is clicked", async () => {
    const onResolve = vi.fn()
    const screen = await render(
      <ApprovalCard
        approval={commandApproval({ allowedScopes: ["run_once", "deny"] })}
        onResolve={onResolve}
      />,
    )
    await screen.getByText("pages.assistant.approval.runOnce").click()
    expect(onResolve).toHaveBeenCalledWith("call-1", true, "run_once")
  })

  it("resolves with approved=false for the deny scope", async () => {
    const onResolve = vi.fn()
    const screen = await render(
      <ApprovalCard
        approval={commandApproval({ allowedScopes: ["run_once", "deny"] })}
        onResolve={onResolve}
      />,
    )
    await screen.getByText("pages.assistant.actions.reject").click()
    expect(onResolve).toHaveBeenCalledWith("call-1", false, "deny")
  })

  it("disables all buttons (and shows a spinner) while a resolution is pending", async () => {
    let resolvePromise: (() => void) | undefined
    const onResolve = vi.fn(
      () => new Promise<void>((r) => {
        resolvePromise = () => r()
      }),
    )
    const screen = await render(
      <ApprovalCard
        approval={commandApproval({ allowedScopes: ["run_once", "deny"] })}
        onResolve={onResolve}
      />,
    )
    await screen.getByText("pages.assistant.approval.runOnce").click()
    await expect.element(screen.getByTestId("spinner")).toBeInTheDocument()
    expect(
      screen.getByRole("button").elements().every((b) => (b as HTMLButtonElement).disabled),
    ).toBe(true)

    resolvePromise?.()
    for (const button of screen.getByRole("button").all()) {
      await expect.element(button).toBeEnabled()
    }
  })
})

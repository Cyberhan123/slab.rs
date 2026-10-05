import { describe, expect, it, vi } from "vitest"
import { render } from "vitest-browser-react"
import type { ReactNode } from "react"

import type { ApprovalScope } from "@slab/api/harness"

import { ApprovalCard } from "../approval-banner"
import type { ApprovalRequest } from "@slab/core/harness"

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
  // The questionnaire wrapper imports buttonVariants for its action buttons.
  buttonVariants: () => "",
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
    await render(<ApprovalCard approval={commandApproval()} onResolve={vi.fn<(itemId: string, approved: boolean, scope: ApprovalScope) => void>()} />)
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
        onResolve={vi.fn<(itemId: string, approved: boolean, scope: ApprovalScope) => void>()}
      />,
    )
    const text = document.body.textContent ?? ""
    expect(text).toContain("src/a.ts")
    expect(text).toContain("-old")
    expect(text).toContain("README.md")
    expect(text).toContain("pages.assistant.approval.fileChange")
  })

  it("shows only the server-advertised approve scopes as choices", async () => {
    const screen = await render(
      <ApprovalCard
        approval={commandApproval({ allowedScopes: ["run_once", "deny"] })}
        onResolve={vi.fn<(itemId: string, approved: boolean, scope: ApprovalScope) => void>()}
      />,
    )
    const radios = screen.getByRole("radio").elements()
    // "deny" is the dedicated reject button, never a scope choice.
    expect(radios).toHaveLength(1)
    await expect
      .element(screen.getByRole("radio", { name: "pages.assistant.approval.runOnce" }))
      .toBeInTheDocument()
    await expect.element(screen.getByRole("button")).toHaveTextContent(
      "pages.assistant.actions.reject",
    )
  })

  it("falls back to a single approve choice when no scopes are advertised", async () => {
    const screen = await render(<ApprovalCard approval={commandApproval()} onResolve={vi.fn<(itemId: string, approved: boolean, scope: ApprovalScope) => void>()} />)
    const radios = screen.getByRole("radio").elements()
    expect(radios).toHaveLength(1)
    await expect
      .element(screen.getByRole("radio", { name: "pages.assistant.actions.approve" }))
      .toBeInTheDocument()
    await expect.element(screen.getByRole("button")).toHaveTextContent(
      "pages.assistant.actions.reject",
    )
  })

  it("resolves immediately with (itemId, approved, scope) when a choice is picked", async () => {
    const onResolve = vi.fn<(itemId: string, approved: boolean, scope: ApprovalScope) => void>()
    const screen = await render(
      <ApprovalCard
        approval={commandApproval({ allowedScopes: ["run_once", "always", "deny"] })}
        onResolve={onResolve}
      />,
    )
    await screen.getByRole("radio", { name: "pages.assistant.approval.always" }).click()
    expect(onResolve).toHaveBeenCalledWith("call-1", true, "always")
  })

  it("resolves with approved=false for the deny button", async () => {
    const onResolve = vi.fn<(itemId: string, approved: boolean, scope: ApprovalScope) => void>()
    const screen = await render(
      <ApprovalCard
        approval={commandApproval({ allowedScopes: ["run_once", "deny"] })}
        onResolve={onResolve}
      />,
    )
    await screen.getByRole("button", { name: "pages.assistant.actions.reject" }).click()
    expect(onResolve).toHaveBeenCalledWith("call-1", false, "deny")
  })

  it("disables the choices and the deny button (with a spinner) while a resolution is pending", async () => {
    let resolvePromise: (() => void) | undefined
    const onResolve = vi.fn<(itemId: string, approved: boolean, scope: ApprovalScope) => void>(() =>
        new Promise<void>((r) => {
          resolvePromise = () => r()
        }),
    )
    const screen = await render(
      <ApprovalCard
        approval={commandApproval({ allowedScopes: ["run_once", "deny"] })}
        onResolve={onResolve}
      />,
    )
    await screen.getByRole("radio", { name: "pages.assistant.approval.runOnce" }).click()
    await expect.element(screen.getByTestId("spinner")).toBeInTheDocument()
    await expect.element(screen.getByRole("radio")).toBeDisabled()
    await expect.element(screen.getByRole("button")).toBeDisabled()

    resolvePromise?.()
    await expect.element(screen.getByRole("radio")).toBeEnabled()
    await expect.element(screen.getByRole("button")).toBeEnabled()
  })
})

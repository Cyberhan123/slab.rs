import { userEvent } from "vitest/browser"
import { render } from "vitest-browser-react"
import type { ComponentProps, ReactNode } from "react"
import { beforeEach, describe, expect, it, vi } from "vitest"


import { WorkspaceGitPanel } from "../workspace-git-panel"

type GitPanelProps = ComponentProps<typeof WorkspaceGitPanel>

const confirmMock = vi.hoisted(() => ({
  confirm: vi.fn<(options: unknown) => Promise<boolean>>().mockResolvedValue(true),
}))

vi.mock("@slab/i18n", async () => {
  const { setupSlabI18nMock } = await import("@slab/test-utils/mocks")
  return setupSlabI18nMock()
})

vi.mock("@slab/components/button", () => ({
  Button: ({
    children,
    onClick,
    disabled,
    type,
    title,
    "aria-label": ariaLabel,
  }: {
    children: ReactNode
    onClick?: () => void
    disabled?: boolean
    type?: "button" | "submit"
    title?: string
    "aria-label"?: string
  }) => (
    <button type={type} onClick={onClick} disabled={disabled} title={title} aria-label={ariaLabel}>
      {children}
    </button>
  ),
}))

vi.mock("../../hooks/use-workspace-confirm", () => ({
  useWorkspaceConfirmDialog: () => ({
    confirm: confirmMock.confirm,
    confirmOpen: false,
    dialog: <div data-testid="workspace-confirm-dialog" />,
  }),
}))

function gitStatus(overrides: Record<string, unknown> = {}) {
  return {
    available: true,
    isRepository: true,
    branch: "main",
    message: null,
    summary: { added: 1, modified: 0, deleted: 0, renamed: 0, copied: 0, untracked: 1, conflicted: 0 },
    entries: [
      { path: "staged.ts", originalPath: null, status: "added", staged: true },
      { path: "unstaged.ts", originalPath: null, status: "modified", staged: false },
    ],
    ...overrides,
  }
}

function baseProps(overrides: Record<string, unknown> = {}): GitPanelProps {
  return {
    gitStatus: gitStatus(),
    gitStatusFetching: false,
    operationPending: false,
    onCommit: vi.fn<(message: string) => Promise<void>>().mockResolvedValue(undefined),
    onDiscard: vi.fn<(path: string) => Promise<void>>().mockResolvedValue(undefined),
    onRefresh: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
    onSelectDiff: vi.fn<(entry: unknown) => void>(),
    onStage: vi.fn<(path: string) => Promise<void>>().mockResolvedValue(undefined),
    selectedEntry: null,
    onUnstage: vi.fn<(path: string) => Promise<void>>().mockResolvedValue(undefined),
    ...overrides,
  } as GitPanelProps
}

describe("WorkspaceGitPanel", () => {
  beforeEach(() => {
    confirmMock.confirm.mockClear()
    confirmMock.confirm.mockResolvedValue(true)
  })

  it("shows a spinner while git status is loading", async () => {
    const screen = await render(<WorkspaceGitPanel {...baseProps({ gitStatus: undefined })} />)

    await expect.element(
      screen.getByLabelText("pages.workspace.git.commitPlaceholder"),
    ).not.toBeInTheDocument()
  })

  it("shows the not-a-repository empty state", async () => {
    const screen = await render(
      <WorkspaceGitPanel
        {...baseProps({ gitStatus: gitStatus({ available: true, isRepository: false, message: "no repo" }) })}
      />,
    )

    await expect.element(screen.getByText("no repo")).toBeInTheDocument()
  })

  it("submits a trimmed commit message and clears the input", async () => {
    const onCommit = vi.fn<(message: string) => Promise<void>>().mockResolvedValue(undefined)
    const screen = await render(<WorkspaceGitPanel {...baseProps({ onCommit })} />)

    await userEvent.type(screen.getByLabelText("pages.workspace.git.commitPlaceholder"), "  fix: bug  ")
    await userEvent.click(screen.getByRole("button", { name: "pages.workspace.git.commit" }))

    await vi.waitFor(() => {
      expect(onCommit).toHaveBeenCalledExactlyOnceWith("fix: bug")
    })
    await expect.element(screen.getByLabelText("pages.workspace.git.commitPlaceholder")).toHaveValue("")
  })

  it("stages an unstaged entry", async () => {
    const onStage = vi.fn<(path: string) => Promise<void>>().mockResolvedValue(undefined)
    const screen = await render(<WorkspaceGitPanel {...baseProps({ onStage })} />)

    await userEvent.click(screen.getByTitle("pages.workspace.git.stage"))

    expect(onStage).toHaveBeenCalledExactlyOnceWith("unstaged.ts")
  })

  it("unstages a staged entry", async () => {
    const onUnstage = vi.fn<(path: string) => Promise<void>>().mockResolvedValue(undefined)
    const screen = await render(<WorkspaceGitPanel {...baseProps({ onUnstage })} />)

    await userEvent.click(screen.getByTitle("pages.workspace.git.unstage"))

    expect(onUnstage).toHaveBeenCalledExactlyOnceWith("staged.ts")
  })

  it("discards an entry only after the confirm dialog accepts", async () => {
    const onDiscard = vi.fn<(path: string) => Promise<void>>().mockResolvedValue(undefined)
    const screen = await render(<WorkspaceGitPanel {...baseProps({ onDiscard })} />)

    // Two entries each expose a discard button; the unstaged one is second in DOM order.
    await userEvent.click(screen.getByTitle("pages.workspace.git.discard").all()[1])

    await vi.waitFor(() => {
      expect(confirmMock.confirm).toHaveBeenCalledOnce()
    })
    await vi.waitFor(() => {
      expect(onDiscard).toHaveBeenCalledExactlyOnceWith("unstaged.ts")
    })
  })
})

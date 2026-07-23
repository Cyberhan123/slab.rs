import { render, screen, waitFor } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import type { ComponentProps, ReactNode } from "react"
import { beforeEach, describe, expect, it, vi } from "vitest"

import { setupSlabI18nMock } from "@slab/test-utils/mocks"

import { WorkspaceGitPanel } from "../workspace-git-panel"

type GitPanelProps = ComponentProps<typeof WorkspaceGitPanel>

const confirmMock = vi.hoisted(() => ({
  confirm: vi.fn<(options: unknown) => Promise<boolean>>().mockResolvedValue(true),
}))

vi.mock("@slab/i18n", () => setupSlabI18nMock())

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

  it("shows a spinner while git status is loading", () => {
    render(<WorkspaceGitPanel {...baseProps({ gitStatus: undefined })} />)

    expect(screen.queryByLabelText("pages.workspace.git.commitPlaceholder")).not.toBeInTheDocument()
  })

  it("shows the not-a-repository empty state", () => {
    render(
      <WorkspaceGitPanel
        {...baseProps({ gitStatus: gitStatus({ available: true, isRepository: false, message: "no repo" }) })}
      />,
    )

    expect(screen.getByText("no repo")).toBeInTheDocument()
  })

  it("submits a trimmed commit message and clears the input", async () => {
    const user = userEvent.setup()
    const onCommit = vi.fn<(message: string) => Promise<void>>().mockResolvedValue(undefined)
    render(<WorkspaceGitPanel {...baseProps({ onCommit })} />)

    await user.type(screen.getByLabelText("pages.workspace.git.commitPlaceholder"), "  fix: bug  ")
    await user.click(screen.getByRole("button", { name: "pages.workspace.git.commit" }))

    await waitFor(() => {
      expect(onCommit).toHaveBeenCalledExactlyOnceWith("fix: bug")
    })
    await waitFor(() => {
      expect(screen.getByLabelText("pages.workspace.git.commitPlaceholder")).toHaveValue("")
    })
  })

  it("stages an unstaged entry", async () => {
    const user = userEvent.setup()
    const onStage = vi.fn<(path: string) => Promise<void>>().mockResolvedValue(undefined)
    render(<WorkspaceGitPanel {...baseProps({ onStage })} />)

    await user.click(screen.getByTitle("pages.workspace.git.stage"))

    expect(onStage).toHaveBeenCalledExactlyOnceWith("unstaged.ts")
  })

  it("unstages a staged entry", async () => {
    const user = userEvent.setup()
    const onUnstage = vi.fn<(path: string) => Promise<void>>().mockResolvedValue(undefined)
    render(<WorkspaceGitPanel {...baseProps({ onUnstage })} />)

    await user.click(screen.getByTitle("pages.workspace.git.unstage"))

    expect(onUnstage).toHaveBeenCalledExactlyOnceWith("staged.ts")
  })

  it("discards an entry only after the confirm dialog accepts", async () => {
    const user = userEvent.setup()
    const onDiscard = vi.fn<(path: string) => Promise<void>>().mockResolvedValue(undefined)
    render(<WorkspaceGitPanel {...baseProps({ onDiscard })} />)

    // Two entries each expose a discard button; the unstaged one is second in DOM order.
    await user.click(screen.getAllByTitle("pages.workspace.git.discard")[1])

    await waitFor(() => {
      expect(confirmMock.confirm).toHaveBeenCalledOnce()
    })
    await waitFor(() => {
      expect(onDiscard).toHaveBeenCalledExactlyOnceWith("unstaged.ts")
    })
  })
})

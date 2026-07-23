import { screen, waitFor } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import type { ComponentProps, ReactNode } from "react"
import { beforeEach, describe, expect, it, vi } from "vitest"

import { renderWithProviders } from "@slab/test-utils/providers/render-with-providers"
import { setupSlabI18nMock } from "@slab/test-utils/mocks"

import { WorkspaceCommandPalette } from "../workspace-command-palette"

type PaletteProps = ComponentProps<typeof WorkspaceCommandPalette>

const searchFilesMock = vi.hoisted(() => ({
  searchFiles: vi.fn<(query: string) => Promise<unknown>>(),
}))

vi.mock("@slab/i18n", () => setupSlabI18nMock())

vi.mock("@/lib/workspace-bridge", () => ({
  workspaceSearchFiles: searchFilesMock.searchFiles,
}))

vi.mock("@slab/components/command", () => ({
  CommandDialog: ({
    open,
    children,
  }: {
    open: boolean
    children: ReactNode
    onOpenChange: (open: boolean) => void
  }) => (open ? <div data-testid="command-dialog">{children}</div> : null),
  CommandInput: ({
    value,
    onValueChange,
  }: {
    value: string
    onValueChange?: (value: string) => void
  }) => (
    <input
      data-testid="command-input"
      aria-label="command-input"
      value={value}
      onChange={(event) => onValueChange?.(event.target.value)}
    />
  ),
  CommandList: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  CommandEmpty: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  CommandGroup: ({
    children,
    heading,
  }: { children: ReactNode; heading?: string } & Record<string, unknown>) => (
    <section data-testid="command-group" data-heading={heading}>
      {children}
    </section>
  ),
  CommandSeparator: () => <hr />,
  CommandItem: ({
    children,
    onSelect,
    value,
    disabled,
  }: {
    children: ReactNode
    onSelect?: () => void
    value?: string
    disabled?: boolean
  }) => (
    <button
      type="button"
      data-testid="command-item"
      data-value={value}
      disabled={disabled}
      onClick={() => onSelect?.()}
    >
      {children}
    </button>
  ),
}))

function baseProps(overrides: Record<string, unknown> = {}): PaletteProps {
  return {
    open: true,
    onOpenChange: vi.fn<(open: boolean) => void>(),
    workspaceRoot: "/proj",
    recentWorkspaces: [],
    openFileTabs: [],
    explorerPanel: "files",
    consoleOpen: false,
    markdownMode: "source",
    selectedFile: null,
    selectedFileDirty: false,
    gitStatusFetching: false,
    gitOperationPending: false,
    onOpenFolder: vi.fn<() => void>(),
    onCloseWorkspace: vi.fn<() => void>(),
    onToggleConsole: vi.fn<() => void>(),
    onSelectExplorerPanel: vi.fn<(panel: unknown) => void>(),
    onRefreshGitStatus: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
    onOpenFile: vi.fn<(relativePath: string) => Promise<unknown>>().mockResolvedValue(undefined),
    onSelectFileTab: vi.fn<(relativePath: string) => Promise<void>>().mockResolvedValue(undefined),
    onRevealDirectoryInTree: vi.fn<(relativePath: string) => Promise<unknown>>().mockResolvedValue(undefined),
    onSaveFile: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
    onSetMarkdownMode: vi.fn<(mode: unknown) => void>(),
    onOpenWorkspacePath: vi.fn<(rootPath: string) => Promise<void>>().mockResolvedValue(undefined),
    onEditorAction: vi.fn<(actionId: string) => Promise<void>>().mockResolvedValue(undefined),
    onExplainWithAssistant: vi.fn<() => void>(),
    ...overrides,
  } as PaletteProps
}

describe("WorkspaceCommandPalette", () => {
  beforeEach(() => {
    searchFilesMock.searchFiles.mockReset()
  })

  it("renders nothing when closed", () => {
    renderWithProviders(<WorkspaceCommandPalette {...baseProps({ open: false })} />)

    expect(screen.queryByTestId("command-dialog")).not.toBeInTheDocument()
  })

  it("renders recent workspaces when open and the query is empty", () => {
    renderWithProviders(
      <WorkspaceCommandPalette
        {...baseProps({
          recentWorkspaces: [{ rootPath: "/repos/alpha", name: "Alpha" }] as never,
        })}
      />,
    )

    expect(screen.getByTestId("command-dialog")).toBeInTheDocument()
    expect(screen.getByText("Alpha")).toBeInTheDocument()
  })

  it("searches files as the user types and opens a file result", async () => {
    const user = userEvent.setup()
    const onOpenFile = vi.fn<(relativePath: string) => Promise<unknown>>().mockResolvedValue(undefined)
    searchFilesMock.searchFiles.mockResolvedValue({
      entries: [{ kind: "file", relativePath: "src/a.ts", name: "a.ts" }],
      truncated: false,
    })
    renderWithProviders(<WorkspaceCommandPalette {...baseProps({ onOpenFile })} />)

    await user.type(screen.getByTestId("command-input"), "a")

    await waitFor(() => {
      expect(searchFilesMock.searchFiles).toHaveBeenCalledWith("a")
    })

    await user.click(screen.getByText("a.ts"))

    expect(onOpenFile).toHaveBeenCalledExactlyOnceWith("src/a.ts")
  })

  it("reveals a directory result in the tree instead of opening it", async () => {
    const user = userEvent.setup()
    const onRevealDirectoryInTree = vi.fn<(relativePath: string) => Promise<unknown>>().mockResolvedValue(undefined)
    searchFilesMock.searchFiles.mockResolvedValue({
      entries: [{ kind: "directory", relativePath: "packages/foo", name: "foo" }],
      truncated: false,
    })
    renderWithProviders(
      <WorkspaceCommandPalette {...baseProps({ onRevealDirectoryInTree })} />,
    )

    await user.type(screen.getByTestId("command-input"), "foo")
    await user.click(await screen.findByText("foo"))

    expect(onRevealDirectoryInTree).toHaveBeenCalledExactlyOnceWith("packages/foo")
  })
})

import { beforeEach, describe, expect, it, vi } from "vitest"
// NOTE: `@slab/test-utils` mock factories must be imported BEFORE the modules
// they mock (`sonner`, `@slab/i18n`). vitest hoists `vi.mock` above all imports
// and runs each factory when its target module is first imported, so the
// factory's helper binding must already be initialized — keep these first.
import { setupSlabI18nMock, setupToastMock } from "@slab/test-utils/mocks"
import { renderWithProviders } from "@slab/test-utils/providers/render-with-providers"
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react"
import { toast } from "sonner"

import { useWorkspaceUiStore } from "@/store/useWorkspaceUiStore"
import { ProjectSwitcher, ProjectSwitcherView } from "../project-switcher"

const { mockWorkspaceOpen } = vi.hoisted(() => ({
  mockWorkspaceOpen: vi.fn<() => Promise<unknown>>(),
}))

// `sonner` / `@slab/i18n` mocks are provided by the shared test-utils factories.
// Handles come back through the re-imported `toast` (and would via `vi.mocked()`
// for i18n), exactly as before — the factory just centralizes the wiring.
vi.mock("sonner", () => setupToastMock())

vi.mock("@slab/i18n", () => setupSlabI18nMock())

vi.mock("@/lib/workspace-bridge", () => ({
  WORKSPACE_STATE_QUERY_KEY: ["workspace-state"],
  workspaceOpen: mockWorkspaceOpen,
}))

const labels = { toggle: "Switch workspace", noActive: "No workspace" }

beforeEach(() => {
  vi.clearAllMocks()
  mockWorkspaceOpen.mockReset()
  useWorkspaceUiStore.setState({
    recentWorkspaces: [],
    workspaces: {},
  })
})

describe("ProjectSwitcherView", () => {
  it("lists recent workspaces and fires onSwitch with the root path", () => {
    const onSwitch = vi.fn<(rootPath: string) => void>()
    render(
      <ProjectSwitcherView
        activeName="Slab"
        labels={labels}
        recentWorkspaces={[
          { rootPath: "repo-a", name: "Alpha" },
          { rootPath: "repo-b", name: "Beta" },
        ]}
        onSwitch={onSwitch}
      />
    )

    fireEvent.click(screen.getByLabelText("Switch workspace"))
    fireEvent.click(screen.getByTestId("project-switcher-item-repo-b"))

    expect(onSwitch).toHaveBeenCalledExactlyOnceWith("repo-b")
    cleanup()
  })

  it("toggles the listbox aria state and shows the active name", () => {
    render(
      <ProjectSwitcherView
        activeName="Active"
        labels={labels}
        recentWorkspaces={[]}
        onSwitch={() => {}}
      />
    )

    const toggle = screen.getByLabelText("Switch workspace")
    expect(toggle.getAttribute("aria-expanded")).toBe("false")
    expect(screen.getByText("Active")).toBeDefined()
    fireEvent.click(toggle)
    expect(toggle.getAttribute("aria-expanded")).toBe("true")
    // No recent workspaces ⇒ the listbox is not rendered.
    expect(screen.queryByTestId("project-switcher-list")).toBeNull()
    cleanup()
  })
})

describe("ProjectSwitcher", () => {
  it("switches through the server open endpoint and reports suspended tasks", async () => {
    mockWorkspaceOpen.mockResolvedValue({
      current: {
        name: "Beta",
        rootPath: "repo-b",
      },
      recent: [],
      migrated: { projectId: "project-1", suspendedCount: 2 },
    })
    useWorkspaceUiStore.setState({
      recentWorkspaces: [
        { rootPath: "repo-a", name: "Alpha", lastOpenedAt: 2 },
        { rootPath: "repo-b", name: "Beta", lastOpenedAt: 1 },
      ],
    })

    const { queryClient } = renderWithProviders(<ProjectSwitcher activeName="Alpha" />)

    fireEvent.click(screen.getByLabelText("pages.workspace.projectSwitcher.toggle"))
    await act(async () => {
      fireEvent.click(screen.getByTestId("project-switcher-item-repo-b"))
    })

    await vi.waitFor(() => {
      expect(mockWorkspaceOpen).toHaveBeenCalledExactlyOnceWith("repo-b")
    })
    await vi.waitFor(() => {
      expect(toast.success).toHaveBeenCalledOnce()
      expect(queryClient.getQueryData(["workspace-state"])).toEqual({
        current: {
          name: "Beta",
          rootPath: "repo-b",
        },
        recent: [],
        migrated: { projectId: "project-1", suspendedCount: 2 },
      })
    })
    expect(toast.success).toHaveBeenCalledExactlyOnceWith(
      "pages.workspace.projectSwitcher.switched",
      {
        description: "pages.workspace.projectSwitcher.suspended:2",
      }
    )
    cleanup()
  })
})

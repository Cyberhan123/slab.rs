import { beforeEach, describe, expect, it, vi } from "vitest"
// NOTE: `@slab/test-utils` mock factories must be imported BEFORE the modules
// they mock (`sonner`, `@slab/i18n`). vitest hoists `vi.mock` above all imports
// and runs each factory when its target module is first imported, so the
// factory's helper binding must already be initialized — keep these first.
import { setupSlabI18nMock, setupToastMock } from "@slab/test-utils/mocks"
import { renderWithProviders } from "@slab/test-utils/providers/render-with-providers"
import { render } from "vitest-browser-react"
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
  it("lists recent workspaces and fires onSwitch with the root path", async () => {
    const onSwitch = vi.fn<(rootPath: string) => void>()
    const screen = await render(
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

    await screen.getByLabelText("Switch workspace").click()
    await screen.getByTestId("project-switcher-item-repo-b").click()

    expect(onSwitch).toHaveBeenCalledExactlyOnceWith("repo-b")
  })

  it("toggles the listbox aria state and shows the active name", async () => {
    const screen = await render(
      <ProjectSwitcherView
        activeName="Active"
        labels={labels}
        recentWorkspaces={[]}
        onSwitch={() => {}}
      />
    )

    const toggle = screen.getByLabelText("Switch workspace")
    expect(toggle.element().getAttribute("aria-expanded")).toBe("false")
    await expect.element(screen.getByText("Active")).toBeInTheDocument()
    await toggle.click()
    expect(toggle.element().getAttribute("aria-expanded")).toBe("true")
    // No recent workspaces ⇒ the listbox is not rendered.
    expect(screen.getByTestId("project-switcher-list").query()).toBeNull()
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

    const screen = await renderWithProviders(<ProjectSwitcher activeName="Alpha" />)

    await screen.getByLabelText("pages.workspace.projectSwitcher.toggle").click()
    await screen.getByTestId("project-switcher-item-repo-b").click()

    await vi.waitFor(() => {
      expect(mockWorkspaceOpen).toHaveBeenCalledExactlyOnceWith("repo-b")
    })
    await vi.waitFor(() => {
      expect(toast.success).toHaveBeenCalledOnce()
      expect(screen.queryClient.getQueryData(["workspace-state"])).toEqual({
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
  })
})

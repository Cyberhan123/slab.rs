import { beforeEach, describe, expect, it, vi } from "vitest"
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react"
import { invoke } from "@tauri-apps/api/core"
import { toast } from "sonner"

import { useWorkspaceUiStore } from "@/store/useWorkspaceUiStore"
import { ProjectSwitcher, ProjectSwitcherView } from "../project-switcher"

const { mockToastSuccess, mockWorkspaceState } = vi.hoisted(() => ({
  mockToastSuccess: vi.fn<() => void>(),
  mockWorkspaceState: vi.fn<() => Promise<unknown>>(),
}))

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn<() => Promise<unknown>>(),
}))

vi.mock("sonner", () => ({
  toast: {
    success: mockToastSuccess,
  },
}))

vi.mock("@slab/i18n", () => ({
  useTranslation: vi.fn<() => { t: (key: string, options?: { count?: number }) => string }>(() => ({
    t: (key, options) => (options ? `${key}:${options.count}` : key),
  })),
}))

vi.mock("@/lib/workspace-bridge", () => ({
  WORKSPACE_STATE_QUERY_KEY: ["workspace-state"],
  workspaceState: mockWorkspaceState,
}))

const labels = { toggle: "Switch workspace", noActive: "No workspace" }

beforeEach(() => {
  vi.clearAllMocks()
  mockWorkspaceState.mockReset()
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
  it("switches through the host migration command and reports suspended tasks", async () => {
    const queryClient = new QueryClient({
      defaultOptions: {
        queries: {
          retry: false,
        },
      },
    })
    vi.mocked(invoke).mockResolvedValue({
      projectId: "project-1",
      suspendedCount: 2,
    })
    mockWorkspaceState.mockResolvedValue({
      current: {
        name: "Beta",
        rootPath: "repo-b",
      },
      recent: [],
    })
    useWorkspaceUiStore.setState({
      recentWorkspaces: [
        { rootPath: "repo-a", name: "Alpha", lastOpenedAt: 2 },
        { rootPath: "repo-b", name: "Beta", lastOpenedAt: 1 },
      ],
    })

    render(
      <QueryClientProvider client={queryClient}>
        <ProjectSwitcher activeName="Alpha" />
      </QueryClientProvider>
    )

    fireEvent.click(screen.getByLabelText("pages.workspace.projectSwitcher.toggle"))
    await act(async () => {
      fireEvent.click(screen.getByTestId("project-switcher-item-repo-b"))
    })

    await vi.waitFor(() => {
      expect(invoke).toHaveBeenCalledExactlyOnceWith("switch_workspace_with_migration", {
        newRoot: "repo-b",
      })
    })
    await vi.waitFor(() => {
      expect(toast.success).toHaveBeenCalledOnce()
      expect(queryClient.getQueryData(["workspace-state"])).toEqual({
        current: {
          name: "Beta",
          rootPath: "repo-b",
        },
        recent: [],
      })
    })
    expect(mockWorkspaceState).toHaveBeenCalledOnce()
    expect(toast.success).toHaveBeenCalledExactlyOnceWith(
      "pages.workspace.projectSwitcher.switched",
      {
        description: "pages.workspace.projectSwitcher.suspended:2",
      }
    )
    cleanup()
  })
})

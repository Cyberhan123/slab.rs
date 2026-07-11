import { beforeEach, describe, expect, it, vi } from "vitest"
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react"
import { toast } from "sonner"

import { useWorkspaceUiStore } from "@/store/useWorkspaceUiStore"
import { ProjectSwitcher, ProjectSwitcherView } from "../project-switcher"

const { mockToastError, mockToastSuccess, mockWorkspaceOpen } = vi.hoisted(() => ({
  mockToastError: vi.fn<() => void>(),
  mockToastSuccess: vi.fn<() => void>(),
  mockWorkspaceOpen: vi.fn<() => Promise<unknown>>(),
}))

vi.mock("sonner", () => ({
  toast: {
    success: mockToastSuccess,
    error: mockToastError,
  },
}))

vi.mock("@slab/i18n", () => ({
  // `default` is the i18next instance; the server-backed UI-state store uses it
  // to render persistence-failure messages, so expose a passthrough `t`.
  default: { t: (key: string) => key },
  useTranslation: vi.fn<() => { t: (key: string, options?: { count?: number }) => string }>(() => ({
    t: (key, options) => (options ? `${key}:${options.count}` : key),
  })),
}))

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
    const queryClient = new QueryClient({
      defaultOptions: {
        queries: {
          retry: false,
        },
      },
    })
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

import { fireEvent, render, screen, waitFor } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"

import { useAgentSurfaceStore } from "@/store/useAgentSurfaceStore"

import { AgentActionCard } from "../agent-action-card"

const mockWorkspaceValidatePath = vi.hoisted(() =>
  vi.fn<(relativePath: string) => Promise<{ relativePath: string }>>()
)

vi.mock("@/lib/workspace-bridge", () => ({
  workspaceValidatePath: mockWorkspaceValidatePath,
}))

const labels = {
  blockedPath: "Blocked path",
  feedback: "Follow up",
  open: "Open",
  review: "Review",
  title: "Artifacts ready",
}

describe("AgentActionCard", () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockWorkspaceValidatePath.mockImplementation(async (relativePath) => ({ relativePath }))
    useAgentSurfaceStore.setState({
      draft: null,
      pendingSurface: null,
    })
  })

  it("validates and dispatches open and review actions for workspace-safe artifact paths", async () => {
    render(
      <AgentActionCard
        artifactRefs={[{ kind: "file", path: "src/main.rs" }]}
        labels={labels}
        onFeedback={vi.fn<() => void>()}
      />
    )

    fireEvent.click(screen.getByTestId("agent-action-open"))
    await waitFor(() => {
      expect(useAgentSurfaceStore.getState().pendingSurface).toMatchObject({
        payload: {
          revealPath: "src/main.rs",
        },
        type: "workspace",
      })
    })
    expect(mockWorkspaceValidatePath).toHaveBeenCalledWith("src/main.rs")

    fireEvent.click(screen.getByTestId("agent-action-review"))
    await waitFor(() => {
      expect(useAgentSurfaceStore.getState().pendingSurface).toMatchObject({
        payload: {
          path: "src/main.rs",
        },
        type: "review",
      })
    })
  })

  it("uses the backend-normalized path before dispatching actions", async () => {
    mockWorkspaceValidatePath.mockResolvedValueOnce({ relativePath: "src/main.rs" })
    render(
      <AgentActionCard
        artifactRefs={[{ kind: "file", path: "src\\main.rs" }]}
        labels={labels}
        onFeedback={vi.fn<() => void>()}
      />
    )

    fireEvent.click(screen.getByTestId("agent-action-open"))
    await waitFor(() => {
      expect(useAgentSurfaceStore.getState().pendingSurface).toMatchObject({
        payload: {
          revealPath: "src/main.rs",
        },
        type: "workspace",
      })
    })
    expect(mockWorkspaceValidatePath).toHaveBeenCalledWith("src/main.rs")
  })

  it("blocks absolute, drive-qualified, or parent-traversing artifact paths", () => {
    const { rerender } = render(
      <AgentActionCard
        artifactRefs={[{ kind: "file", path: "C:/Users/example/.ssh/id_rsa" }]}
        labels={labels}
        onFeedback={vi.fn<() => void>()}
      />
    )

    expect(screen.getByTestId("agent-action-open")).toBeDisabled()
    expect(screen.getByTestId("agent-action-review")).toBeDisabled()
    expect(screen.getByText("Blocked path")).toBeInTheDocument()

    rerender(
      <AgentActionCard
        artifactRefs={[{ kind: "file", path: "\\Users\\example\\.ssh\\id_rsa" }]}
        labels={labels}
        onFeedback={vi.fn<() => void>()}
      />
    )

    expect(screen.getByTestId("agent-action-open")).toBeDisabled()

    rerender(
      <AgentActionCard
        artifactRefs={[{ kind: "file", path: "C:relative.txt" }]}
        labels={labels}
        onFeedback={vi.fn<() => void>()}
      />
    )

    expect(screen.getByTestId("agent-action-open")).toBeDisabled()
  })

  it("does not dispatch when workspace validation rejects the path", async () => {
    mockWorkspaceValidatePath.mockRejectedValueOnce(new Error("outside workspace"))
    render(
      <AgentActionCard
        artifactRefs={[{ kind: "file", path: "src/main.rs" }]}
        labels={labels}
        onFeedback={vi.fn<() => void>()}
      />
    )

    fireEvent.click(screen.getByTestId("agent-action-open"))

    await waitFor(() => {
      expect(screen.getByText("Blocked path")).toBeInTheDocument()
    })
    expect(useAgentSurfaceStore.getState().pendingSurface).toBeNull()
  })

  it("feeds follow-up prompts back to the caller", () => {
    const onFeedback = vi.fn<(prompt: string) => void>()
    render(
      <AgentActionCard
        artifactRefs={[{ kind: "file", path: "../outside.txt" }]}
        labels={labels}
        onFeedback={onFeedback}
      />
    )

    fireEvent.click(screen.getByTestId("agent-action-feedback"))

    expect(onFeedback).toHaveBeenCalledWith("Continue from ../outside.txt")
  })
})

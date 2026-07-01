import { fireEvent, render, screen, waitFor } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"
import { MemoryRouter } from "react-router-dom"

import { useAgentSurfaceStore } from "@/store/useAgentSurfaceStore"
import { AgentSurfaceLayer } from "../agent-surface-layer"

const mockNavigate = vi.hoisted(() => vi.fn<() => void>())

vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual<typeof import("react-router-dom")>("react-router-dom")
  return {
    ...actual,
    useNavigate: () => mockNavigate,
  }
})

function renderLayer(onSurfaceClosed = vi.fn<() => void>()) {
  render(
    <MemoryRouter>
      <AgentSurfaceLayer onSurfaceClosed={onSurfaceClosed} />
    </MemoryRouter>
  )
  return onSurfaceClosed
}

describe("AgentSurfaceLayer", () => {
  beforeEach(() => {
    vi.clearAllMocks()
    useAgentSurfaceStore.setState({
      draft: null,
      focusComposerSignal: 0,
      pendingSurface: null,
    })
  })

  it("consumes pending inline workspace surfaces", async () => {
    useAgentSurfaceStore.getState().setPendingSurface({
      type: "workspace",
      payload: {
        revealPath: "src/main.rs",
      },
    })

    renderLayer()

    await waitFor(() => {
      expect(screen.getByTestId("a2u-workspace-surface")).toHaveTextContent("src/main.rs")
    })
    expect(useAgentSurfaceStore.getState().pendingSurface).toBeNull()
  })

  it("keeps workspace-targeted reveal surfaces pending for the workspace page", () => {
    useAgentSurfaceStore.getState().setPendingSurface(
      {
        type: "workspace",
        payload: {
          revealPath: "src/lib.rs",
        },
      },
      { targetRoute: "workspace" }
    )

    renderLayer()

    expect(screen.queryByTestId("agent-surface-layer")).toBeNull()
    expect(useAgentSurfaceStore.getState().pendingSurface).toMatchObject({
      type: "workspace",
      targetRoute: "workspace",
      payload: {
        revealPath: "src/lib.rs",
      },
    })
  })

  it("re-queues workspace reveal requests and closes the preview before navigation", async () => {
    useAgentSurfaceStore.getState().setPendingSurface({
      type: "workspace",
      payload: {
        revealPath: "src/main.rs",
      },
    })
    const onSurfaceClosed = renderLayer()

    await waitFor(() => {
      expect(screen.getByTestId("agent-surface-open-workspace")).toBeInTheDocument()
    })
    fireEvent.click(screen.getByTestId("agent-surface-open-workspace"))

    expect(onSurfaceClosed).toHaveBeenCalledOnce()
    expect(mockNavigate).toHaveBeenCalledExactlyOnceWith("/workspace")
    expect(useAgentSurfaceStore.getState().pendingSurface).toMatchObject({
      type: "workspace",
      targetRoute: "workspace",
      payload: {
        revealPath: "src/main.rs",
      },
    })
  })
})

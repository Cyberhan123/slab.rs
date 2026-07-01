import { beforeEach, describe, expect, it } from "vitest"

import { useAgentSurfaceStore } from "../useAgentSurfaceStore"

describe("useAgentSurfaceStore", () => {
  beforeEach(() => {
    useAgentSurfaceStore.setState({
      draft: null,
      focusComposerSignal: 0,
      pendingSurface: null,
    })
  })

  it("stores and consumes assistant drafts once", () => {
    useAgentSurfaceStore.getState().setDraft({
      autoSubmit: false,
      prompt: "Explain this file",
      source: {
        label: "main.rs",
        path: "src/main.rs",
      },
    })

    const draft = useAgentSurfaceStore.getState().consumeDraft()

    expect(draft).toEqual({
      autoSubmit: false,
      prompt: "Explain this file",
      source: {
        label: "main.rs",
        path: "src/main.rs",
      },
    })
    expect(useAgentSurfaceStore.getState().draft).toBeNull()
    expect(useAgentSurfaceStore.getState().consumeDraft()).toBeNull()
  })

  it("stores a typed pending workspace surface", () => {
    useAgentSurfaceStore.getState().setPendingSurface(
      {
        type: "workspace",
        payload: {
          revealPath: "C:/work/slab/src/main.rs",
        },
      },
      { targetRoute: "workspace" }
    )

    const surface = useAgentSurfaceStore.getState().pendingSurface

    expect(surface).toMatchObject({
      type: "workspace",
      payload: {
        revealPath: "C:/work/slab/src/main.rs",
      },
      targetRoute: "workspace",
    })
    expect(surface?.id).toMatch(/^workspace:/)
    expect(typeof surface?.createdAt).toBe("number")
  })

  it("only consumes the matching pending surface request", () => {
    useAgentSurfaceStore.getState().setPendingSurface({
      type: "workspace",
      payload: {
        revealPath: "src/lib.rs",
      },
    })

    const surface = useAgentSurfaceStore.getState().pendingSurface

    expect(useAgentSurfaceStore.getState().consumePendingSurface("other")).toBeNull()
    expect(useAgentSurfaceStore.getState().pendingSurface).toBe(surface)
    expect(useAgentSurfaceStore.getState().consumePendingSurface(surface?.id)).toBe(surface)
    expect(useAgentSurfaceStore.getState().pendingSurface).toBeNull()
  })

  it("increments the composer focus signal for shell-owned surface close", () => {
    expect(useAgentSurfaceStore.getState().focusComposerSignal).toBe(0)

    useAgentSurfaceStore.getState().requestComposerFocus()
    useAgentSurfaceStore.getState().requestComposerFocus()

    expect(useAgentSurfaceStore.getState().focusComposerSignal).toBe(2)
  })
})

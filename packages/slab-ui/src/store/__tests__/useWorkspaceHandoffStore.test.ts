import { beforeEach, describe, expect, it } from "vitest"

import { useWorkspaceHandoffStore } from "../useWorkspaceHandoffStore"

describe("useWorkspaceHandoffStore", () => {
  beforeEach(() => {
    useWorkspaceHandoffStore.setState({
      draft: null,
      pendingWorkspaceReveal: null,
    })
  })

  describe("draft handoff", () => {
    it("stores and consumes a draft", () => {
      useWorkspaceHandoffStore.getState().setDraft({
        autoSubmit: false,
        prompt: "draft",
      })
      const draft = useWorkspaceHandoffStore.getState().consumeDraft()

      expect(draft).toEqual({ autoSubmit: false, prompt: "draft" })
      expect(useWorkspaceHandoffStore.getState().draft).toBeNull()
      expect(useWorkspaceHandoffStore.getState().consumeDraft()).toBeNull()
    })

    it("clears a draft", () => {
      useWorkspaceHandoffStore.getState().setDraft({ autoSubmit: false, prompt: "draft" })
      useWorkspaceHandoffStore.getState().clearDraft()

      expect(useWorkspaceHandoffStore.getState().draft).toBeNull()
    })
  })

  describe("pending workspace reveal", () => {
    it("stores a workspace reveal request with a synthetic id", () => {
      useWorkspaceHandoffStore.getState().setPendingWorkspaceReveal({
        type: "workspace",
        payload: { revealPath: "readme.md" },
      })
      const reveal = useWorkspaceHandoffStore.getState().pendingWorkspaceReveal

      expect(reveal).toMatchObject({
        type: "workspace",
        payload: { revealPath: "readme.md" },
      })
      expect(typeof reveal?.id).toBe("string")
    })

    it("consumes the matching reveal by id and leaves others untouched", () => {
      useWorkspaceHandoffStore.getState().setPendingWorkspaceReveal({
        type: "workspace",
        payload: { revealPath: "a" },
      })
      const reveal = useWorkspaceHandoffStore.getState().pendingWorkspaceReveal

      expect(useWorkspaceHandoffStore.getState().consumePendingWorkspaceReveal("other")).toBeNull()
      expect(useWorkspaceHandoffStore.getState().pendingWorkspaceReveal).toBe(reveal)
      expect(useWorkspaceHandoffStore.getState().consumePendingWorkspaceReveal(reveal?.id)).toBe(reveal)
      expect(useWorkspaceHandoffStore.getState().pendingWorkspaceReveal).toBeNull()
    })

    it("clears the reveal by id", () => {
      useWorkspaceHandoffStore.getState().setPendingWorkspaceReveal({
        type: "workspace",
        payload: { revealPath: "a" },
      })
      const reveal = useWorkspaceHandoffStore.getState().pendingWorkspaceReveal

      useWorkspaceHandoffStore.getState().clearPendingWorkspaceReveal("other")
      expect(useWorkspaceHandoffStore.getState().pendingWorkspaceReveal).toBe(reveal)

      useWorkspaceHandoffStore.getState().clearPendingWorkspaceReveal(reveal?.id)
      expect(useWorkspaceHandoffStore.getState().pendingWorkspaceReveal).toBeNull()
    })

    it("clears the reveal without an id", () => {
      useWorkspaceHandoffStore.getState().setPendingWorkspaceReveal({ type: "workspace", payload: {} })
      useWorkspaceHandoffStore.getState().clearPendingWorkspaceReveal()
      expect(useWorkspaceHandoffStore.getState().pendingWorkspaceReveal).toBeNull()
    })

    it("returns null when there is nothing to consume", () => {
      expect(useWorkspaceHandoffStore.getState().consumePendingWorkspaceReveal()).toBeNull()
      expect(useWorkspaceHandoffStore.getState().consumePendingWorkspaceReveal("any-id")).toBeNull()
    })
  })
})

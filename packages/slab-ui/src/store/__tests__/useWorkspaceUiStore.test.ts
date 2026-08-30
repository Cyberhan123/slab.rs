import { beforeEach, describe, expect, it } from "vitest"

import { useWorkspaceUiStore } from "../useWorkspaceUiStore"

describe("useWorkspaceUiStore.assistantPinnedWorkspaceRoot", () => {
    beforeEach(() => {
        useWorkspaceUiStore.setState({ assistantPinnedWorkspaceRoot: null })
    })

    it("starts null and pins a trimmed root", () => {
        expect(useWorkspaceUiStore.getState().assistantPinnedWorkspaceRoot).toBeNull()

        useWorkspaceUiStore.getState().setAssistantPinnedWorkspaceRoot("  C:\\repo  ")
        expect(useWorkspaceUiStore.getState().assistantPinnedWorkspaceRoot).toBe("C:\\repo")
    })

    it("clears on null or blank input", () => {
        useWorkspaceUiStore.getState().setAssistantPinnedWorkspaceRoot("C:\\repo")

        useWorkspaceUiStore.getState().setAssistantPinnedWorkspaceRoot(null)
        expect(useWorkspaceUiStore.getState().assistantPinnedWorkspaceRoot).toBeNull()

        useWorkspaceUiStore.getState().setAssistantPinnedWorkspaceRoot("   ")
        expect(useWorkspaceUiStore.getState().assistantPinnedWorkspaceRoot).toBeNull()
    })
})

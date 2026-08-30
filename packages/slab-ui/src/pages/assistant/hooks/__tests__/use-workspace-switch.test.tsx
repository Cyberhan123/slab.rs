import { renderHook } from "vitest-browser-react"
import { beforeEach, describe, expect, it, vi } from "vitest"

const harness = vi.hoisted(() => ({
    open: vi.fn(),
    close: vi.fn(),
    setQueryData: vi.fn(),
    invalidateQueries: vi.fn().mockResolvedValue(undefined),
    remember: vi.fn(),
    toast: { success: vi.fn(), error: vi.fn() },
}))

vi.mock("@tanstack/react-query", () => ({
    useQueryClient: () => ({
        setQueryData: harness.setQueryData,
        invalidateQueries: harness.invalidateQueries,
    }),
}))

vi.mock("sonner", () => ({ toast: harness.toast }))

vi.mock("@slab/i18n", () => ({
    useTranslation: () => ({ t: (key: string, params?: unknown) => `${key}${params ? JSON.stringify(params) : ""}` }),
}))

vi.mock("@slab/core/workspace/bridge", () => ({
    WORKSPACE_STATE_QUERY_KEY: ["workspace-state"],
    workspaceOpen: harness.open,
    workspaceClose: harness.close,
}))

vi.mock("@slab/ui/store/useWorkspaceUiStore", () => ({
    useWorkspaceUiStore: (selector: (state: { rememberRecentWorkspace: unknown }) => unknown) =>
        selector({ rememberRecentWorkspace: harness.remember }),
}))

import { isSameRoot, useWorkspaceSwitch } from "../use-workspace-switch"

describe("useWorkspaceSwitch.applyWorkspace", () => {
    beforeEach(() => {
        vi.clearAllMocks()
        harness.invalidateQueries.mockResolvedValue(undefined)
    })

    it("is a no-op when the target root already matches the active one", async () => {
        harness.open.mockResolvedValue({ current: { rootPath: "C:\\ws", name: "ws" } })
        const { result } = await renderHook(() => useWorkspaceSwitch())

        const changed = await result.current.applyWorkspace(
            { kind: "root", rootPath: "C:\\ws" },
            "C:/ws", // separator/case differences normalize to the same root
        )

        expect(changed).toBe(false)
        expect(harness.open).not.toHaveBeenCalled()
    })

    it("is a no-op when going global with no active workspace", async () => {
        const { result } = await renderHook(() => useWorkspaceSwitch())
        expect(await result.current.applyWorkspace({ kind: "global" }, null)).toBe(false)
        expect(harness.close).not.toHaveBeenCalled()
    })

    it("opens a different root, remembers it, and refreshes the workspace cache", async () => {
        const state = { current: { rootPath: "C:\\next", name: "next" }, migrated: null }
        harness.open.mockResolvedValue(state)
        const { result } = await renderHook(() => useWorkspaceSwitch())

        const changed = await result.current.applyWorkspace(
            { kind: "root", rootPath: "C:\\next" },
            "C:\\old",
        )

        expect(changed).toBe(true)
        expect(harness.open).toHaveBeenCalledWith("C:\\next")
        expect(harness.remember).toHaveBeenCalledWith({ name: "next", rootPath: "C:\\next" })
        expect(harness.setQueryData).toHaveBeenCalledWith(["workspace-state"], state)
        expect(harness.invalidateQueries).toHaveBeenCalledWith({ queryKey: ["workspace-state"] })
    })

    it("surfaces the migration summary (suspended threads) in the success toast", async () => {
        harness.open.mockResolvedValue({
            current: { rootPath: "C:\\next", name: "next" },
            migrated: { projectId: "p1", suspendedCount: 3 },
        })
        const { result } = await renderHook(() => useWorkspaceSwitch())
        await result.current.applyWorkspace({ kind: "root", rootPath: "C:\\next" }, "C:\\old")

        expect(harness.toast.success).toHaveBeenCalledWith(
            expect.stringContaining("pages.workspace.projectSwitcher.switched"),
            { description: expect.stringContaining("suspended") },
        )
    })

    it("goes global via workspaceClose and toasts the global label", async () => {
        harness.close.mockResolvedValue({ current: null })
        const { result } = await renderHook(() => useWorkspaceSwitch())
        const changed = await result.current.applyWorkspace({ kind: "global" }, "C:\\old")

        expect(changed).toBe(true)
        expect(harness.close).toHaveBeenCalled()
        expect(harness.toast.success).toHaveBeenCalledWith(
            "pages.assistant.newChat.switchedGlobal",
          expect.anything(),
        )
    })

    it("toasts and rethrows on failure so the caller can skip downstream steps", async () => {
        harness.open.mockRejectedValue(new Error("boom"))
        const { result } = await renderHook(() => useWorkspaceSwitch())

        await expect(
            result.current.applyWorkspace({ kind: "root", rootPath: "C:\\bad" }, "C:\\old"),
        ).rejects.toThrow("boom")
        expect(harness.toast.error).toHaveBeenCalledWith(
            "pages.workspace.toast.openFailed",
            expect.objectContaining({ description: "boom" }),
        )
    })
})

describe("isSameRoot", () => {
    it("normalizes separators, trailing slashes, and case", () => {
        expect(isSameRoot("C:\\WS\\", "c:/ws")).toBe(true)
        expect(isSameRoot("/home/user", "/home/user/")).toBe(true)
        expect(isSameRoot("C:\\a", "C:\\b")).toBe(false)
        expect(isSameRoot(null, "C:\\a")).toBe(false)
        expect(isSameRoot(null, undefined)).toBe(false)
    })
})

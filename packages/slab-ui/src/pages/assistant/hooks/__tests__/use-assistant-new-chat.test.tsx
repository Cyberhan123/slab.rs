import { render } from "vitest-browser-react"
import { act, useEffect } from "react"
import { beforeEach, describe, expect, it, vi } from "vitest"

const harness = vi.hoisted(() => ({
    calls: [] as string[],
    navigate: vi.fn(),
    pathname: "/",
    workspaceData: { current: null as { rootPath: string; name: string } | null },
    apply: vi.fn(),
}))

vi.mock("@tanstack/react-query", () => ({
    useQuery: () => ({ data: harness.workspaceData }),
}))

vi.mock("react-router-dom", () => ({
    useNavigate: () => harness.navigate,
    useLocation: () => ({ pathname: harness.pathname }),
}))

vi.mock("@slab/core/workspace/bridge", () => ({
    WORKSPACE_STATE_QUERY_KEY: ["workspace-state"],
    workspaceState: vi.fn(),
}))

vi.mock("../use-workspace-switch", () => ({
    useWorkspaceSwitch: () => ({ applyWorkspace: harness.apply, switching: false }),
    isSameRoot: (left: string | null, right: string | null) =>
        typeof left === "string" && typeof right === "string" && left.toLowerCase() === right.toLowerCase(),
}))

vi.mock("@slab/components/dialog", () => ({
    Dialog: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
    DialogContent: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
    DialogDescription: () => null,
    DialogHeader: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
    DialogTitle: () => null,
}))

const selectorCapture = vi.hoisted(() => ({
    onValueChange: undefined as unknown as (selection: unknown) => void,
}))
vi.mock("@slab/ui/components/workspace-selector", () => ({
    WorkspaceSelector: (props: { onValueChange: (selection: unknown) => void }) => {
        selectorCapture.onValueChange = props.onValueChange
        return <div data-testid="workspace-selector-stub" />
    },
}))

// The embedded Sender is stubbed; its onSubmit capture is how tests drive the
// hook's submit orchestration (exactly what the real dialog forwards).
const senderCapture = vi.hoisted(() => ({
    onSubmit: undefined as unknown as (message: string, options: unknown) => unknown,
}))
vi.mock("@slab/ui/pages/assistant/components/sender.tsx", () => ({
    default: (props: { onSubmit: (message: string, options: unknown) => unknown }) => {
        senderCapture.onSubmit = props.onSubmit
        return <div data-testid="sender-stub" />
    },
}))

import { useAssistantNewChat } from "../use-assistant-new-chat"
import { useWorkspaceHandoffStore } from "@slab/ui/store/useWorkspaceHandoffStore"

type NewChatApi = ReturnType<typeof useAssistantNewChat>

// Live probe: renders the hook's dialog element on every hook render, so the
// stubbed Sender/selector captures the LATEST submit/selection callbacks
// (a static `render(result.current.dialog)` snapshot would pin stale closures).
const hookRef: { current: NewChatApi | null } = { current: null }

function Probe({ createSession }: { createSession: (options?: unknown) => Promise<{ id: string } | null> }) {
    const api = useAssistantNewChat({
        createSession: createSession as never,
        commands: [],
    })
    // Publish the live hook API via an effect (mutating the module-level ref
    // during render trips the react-compiler immutability lint).
    useEffect(() => {
        hookRef.current = api
    })
    return api.dialog
}

async function setup(createSession: (options?: unknown) => Promise<{ id: string } | null>) {
    await render(<Probe createSession={createSession} />)
    return {
        // Both wrap their state updates in  so React flushes them —
        // un-acted updates in the browser test env would leave the captured
        // submit closure stale.
        openDialog: async () => {
            await act(async () => {
                hookRef.current?.openDialog()
            })
        },
        select: async (selection: unknown) => {
            await act(async () => {
                selectorCapture.onValueChange(selection)
            })
        },
    }
}

describe("useAssistantNewChat", () => {
    beforeEach(() => {
        vi.clearAllMocks()
        harness.calls = []
        harness.pathname = "/"
        harness.workspaceData = { current: null }
        useWorkspaceHandoffStore.setState({ draft: null })
    })

    it("cold open: creates the session, deep-links, switches, then stages the draft — in order", async () => {
        const createSession = vi.fn(async () => {
            harness.calls.push("createSession")
            return { id: "s1" }
        })
        harness.apply.mockImplementation(async () => {
            harness.calls.push("applyWorkspace")
            return true
        })
        const navigate = harness.navigate

        const api = await setup(createSession)
        // The user picks a workspace while none is active (cold-loaded "/").
        await api.select({ kind: "root", rootPath: "C:\\ws" })
        await senderCapture.onSubmit("hello workspace", {
            files: [],
            effort: "high",
            permissionMode: "default",
        })

        expect(harness.calls).toEqual(["createSession", "applyWorkspace"])
        // The deep link lands BEFORE the switch flips the workspace cache (the
        // WorkspaceModeSync redirect guard).
        expect(navigate).toHaveBeenCalledWith("/?session=s1", { replace: true })
        expect(harness.apply).toHaveBeenCalledWith({ kind: "root", rootPath: "C:\\ws" }, null)
        const draft = useWorkspaceHandoffStore.getState().draft
        expect(draft).toMatchObject({
            autoSubmit: true,
            prompt: "hello workspace",
            sessionId: "s1",
            effort: "high",
            permissionMode: "default",
        })
    })

    it("keeps the user's workspace selection (recent root, cold load)", async () => {
        const createSession = vi.fn(async () => ({ id: "s2" }))
        const api = await setup(createSession)
        await api.openDialog()
        // openDialog seeds the default (global with no active workspace), and
        // the user submits without changing it: no switch, no navigation.
        await senderCapture.onSubmit("global hello", { files: [], effort: "low", permissionMode: "default" })
        expect(harness.apply).not.toHaveBeenCalled()
        expect(harness.navigate).not.toHaveBeenCalled()
        expect(useWorkspaceHandoffStore.getState().draft).toMatchObject({
            prompt: "global hello",
            sessionId: "s2",
        })
    })

    it("defaults to global even when a workspace is active (closing, no deep link)", async () => {
        harness.workspaceData = { current: { rootPath: "C:\\old", name: "old" } }
        const createSession = vi.fn(async () => ({ id: "s3" }))
        const api = await setup(createSession)
        // openDialog seeds the DEFAULT selection = 全局 (global), not the
        // active workspace — submitting switches to global chat.
        await api.openDialog()

        await senderCapture.onSubmit("switch it", { files: [], effort: "low", permissionMode: "default" })

        // Global selection + active workspace → a real close switch. Closing
        // never deep-links (the WorkspaceModeSync redirect only fires when a
        // workspace OPENS).
        expect(harness.apply).toHaveBeenCalledWith({ kind: "global" }, "C:\\old")
        expect(harness.navigate).not.toHaveBeenCalled()
        expect(useWorkspaceHandoffStore.getState().draft).toMatchObject({ sessionId: "s3" })
    })

    it("does not stage the draft when the workspace switch fails (dialog stays usable)", async () => {
        const createSession = vi.fn(async () => ({ id: "s4" }))
        harness.apply.mockRejectedValue(new Error("open failed"))
        const api = await setup(createSession)
        await api.select({ kind: "root", rootPath: "C:\\broken" })

        await expect(
            senderCapture.onSubmit("risky", { files: [], effort: "low", permissionMode: "default" }),
        ).resolves.toBeUndefined()

        expect(useWorkspaceHandoffStore.getState().draft).toBeNull()
    })

    it("aborts when session creation fails (already toasted by the sessions hook)", async () => {
        const createSession = vi.fn(async () => null)
        await setup(createSession)

        await senderCapture.onSubmit("nope", { files: [], effort: "low", permissionMode: "default" })

        expect(harness.apply).not.toHaveBeenCalled()
        expect(useWorkspaceHandoffStore.getState().draft).toBeNull()
    })
})

import { render } from "vitest-browser-react"
import { act, useEffect } from "react"
import { beforeEach, describe, expect, it, vi } from "vitest"

const harness = vi.hoisted(() => ({
    calls: [] as string[],
    navigate: vi.fn(),
    workspaceData: { current: null as { rootPath: string; name: string } | null },
    apply: vi.fn(),
}))

vi.mock("@tanstack/react-query", () => ({
    useQuery: () => ({ data: harness.workspaceData }),
}))

vi.mock("react-router-dom", () => ({
    useNavigate: () => harness.navigate,
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

const selectorCapture = vi.hoisted(() => ({
    onValueChange: undefined as unknown as (selection: unknown) => void,
}))
vi.mock("@slab/ui/components/workspace-selector", () => ({
    WorkspaceSelector: (props: { onValueChange: (selection: unknown) => void }) => {
        selectorCapture.onValueChange = props.onValueChange
        return <div data-testid="workspace-selector-stub" />
    },
}))

// The landing's embedded Sender is stubbed; its onSubmit capture is how tests
// drive the hook's submit orchestration (exactly what the real landing
// forwards). The REAL Sender seeds its state from `initialValue` on mount and
// keeps it when the prop later turns undefined (the draft claim) — the stub
// mirrors that by only capturing the first non-undefined seed.
const senderCapture = vi.hoisted(() => ({
    initialValue: undefined as unknown as string | undefined,
    onSubmit: undefined as unknown as (message: string, options: unknown) => unknown,
}))
vi.mock("@slab/ui/pages/assistant/components/sender.tsx", () => ({
    default: (props: {
        initialValue?: string
        onSubmit: (message: string, options: unknown) => unknown
    }) => {
        senderCapture.onSubmit = props.onSubmit
        if (senderCapture.initialValue === undefined) {
            senderCapture.initialValue = props.initialValue
        }
        return <div data-testid="sender-stub" />
    },
}))

import { useAssistantNewChat } from "../use-assistant-new-chat"
import { useWorkspaceHandoffStore } from "@slab/ui/store/useWorkspaceHandoffStore"

type NewChatApi = ReturnType<typeof useAssistantNewChat>

// Live probe: renders the hook's landing element on every hook render, so the
// stubbed Sender/selector captures the LATEST submit/selection callbacks (a
// static `render(result.current.landing)` snapshot would pin stale closures).
const hookRef: { current: NewChatApi | null } = { current: null }

const conversations = [
    { key: "session-a", label: "Session A", group: "Workspace" },
    { key: "session-b", label: "Session B", group: "Workspace" },
]

function Probe({ createSession }: { createSession: (options?: unknown) => Promise<{ id: string } | null> }) {
    const api = useAssistantNewChat({
        createSession: createSession as never,
        commands: [],
        active: true,
        conversations,
        onSelectConversation: () => {},
        conversationsBusy: false,
    })
    // Publish the live hook API via an effect (mutating the module-level ref
    // during render trips the react-compiler immutability lint).
    useEffect(() => {
        hookRef.current = api
    })
    return api.landing
}

async function setup(createSession: (options?: unknown) => Promise<{ id: string } | null>) {
    await render(<Probe createSession={createSession} />)
    return {
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
        harness.navigate.mockImplementation(() => {
            harness.calls.push("navigate")
        })
        harness.workspaceData = { current: null }
        senderCapture.initialValue = undefined
        useWorkspaceHandoffStore.setState({ draft: null })
    })

    it("submit: creates the session, deep-links, switches, then stages the draft — in order", async () => {
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
        // The user picks a workspace while none is active (landing on "/").
        await api.select({ kind: "root", rootPath: "C:\\ws" })
        await senderCapture.onSubmit("hello workspace", {
            files: [],
            effort: "high",
            permissionMode: "default",
        })

        expect(harness.calls).toEqual(["createSession", "navigate", "applyWorkspace"])
        // The deep link lands BEFORE the switch flips the workspace cache (the
        // WorkspaceModeSync redirect guard) and turns the page into the detail.
        expect(navigate).toHaveBeenCalledWith("/?session=s1")
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

    it("global submit still deep-links into the detail (no workspace switch)", async () => {
        const createSession = vi.fn(async () => ({ id: "s2" }))
        await setup(createSession)

        await senderCapture.onSubmit("global hello", { files: [], effort: "low", permissionMode: "default" })

        expect(harness.apply).not.toHaveBeenCalled()
        expect(harness.navigate).toHaveBeenCalledWith("/?session=s2")
        expect(useWorkspaceHandoffStore.getState().draft).toMatchObject({
            prompt: "global hello",
            sessionId: "s2",
        })
    })

    it("defaults to global even when a workspace is active (closing switch)", async () => {
        harness.workspaceData = { current: { rootPath: "C:\\old", name: "old" } }
        const createSession = vi.fn(async () => ({ id: "s3" }))
        await setup(createSession)
        // The landing seeds the DEFAULT selection = 全局 (global), not the
        // active workspace — submitting switches to global chat.
        await senderCapture.onSubmit("switch it", { files: [], effort: "low", permissionMode: "default" })

        // Global selection + active workspace → a real close switch. Closing
        // still deep-links (the detail is the destination regardless).
        expect(harness.apply).toHaveBeenCalledWith({ kind: "global" }, "C:\\old")
        expect(harness.navigate).toHaveBeenCalledWith("/?session=s3")
        expect(useWorkspaceHandoffStore.getState().draft).toMatchObject({ sessionId: "s3" })
    })

    it("claims a staged session-less handoff draft and prefills the composer", async () => {
        useWorkspaceHandoffStore.setState({
            draft: {
                autoSubmit: false,
                prompt: "Explain this code from src/app.ts",
            },
        })
        const createSession = vi.fn(async () => ({ id: "s5" }))

        await setup(createSession)

        // The draft is consumed exactly once and surfaces as the Sender's
        // initial text (the landing has no chat pane to auto-send it).
        expect(useWorkspaceHandoffStore.getState().draft).toBeNull()
        await vi.waitFor(() => {
            expect(senderCapture.initialValue).toBe("Explain this code from src/app.ts")
        })
    })

    it("does not stage the draft when the workspace switch fails (nothing stranded)", async () => {
        const createSession = vi.fn(async () => ({ id: "s4" }))
        harness.apply.mockRejectedValue(new Error("open failed"))
        const api = await setup(createSession)
        await api.select({ kind: "root", rootPath: "C:\\broken" })
        await senderCapture.onSubmit("risky", { files: [], effort: "low", permissionMode: "default" })

        expect(useWorkspaceHandoffStore.getState().draft).toBeNull()
    })

    it("aborts when session creation fails (already toasted by the sessions hook)", async () => {
        const createSession = vi.fn(async () => null)
        await setup(createSession)

        await senderCapture.onSubmit("nope", { files: [], effort: "low", permissionMode: "default" })

        expect(harness.apply).not.toHaveBeenCalled()
        expect(harness.navigate).not.toHaveBeenCalled()
        expect(useWorkspaceHandoffStore.getState().draft).toBeNull()
    })
})

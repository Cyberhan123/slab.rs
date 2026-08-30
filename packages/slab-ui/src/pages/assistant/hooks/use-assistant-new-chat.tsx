import { useCallback, useState, type ReactNode } from "react"
import { useQuery } from "@tanstack/react-query"
import { useLocation, useNavigate } from "react-router-dom"

import type { CommandInfo } from "@slab/core/harness"
import { WORKSPACE_STATE_QUERY_KEY, workspaceState } from "@slab/core/workspace/bridge"
import { useWorkspaceHandoffStore } from "@slab/ui/store/useWorkspaceHandoffStore"

import { AssistantNewChatDialog } from "../components/assistant-new-chat-dialog"
import type { SenderSubmitOptions } from "../components/sender"
import { isSameRoot, useWorkspaceSwitch, type WorkspaceSelection } from "./use-workspace-switch"

type CreateSessionFn = (options?: { quiet?: boolean; select?: boolean }) => Promise<
    { id: string } | null
>

type UseAssistantNewChatArgs = {
    /** `useAssistantSessions().createSession` — creates and selects a session. */
    createSession: CreateSessionFn
    /** Command registry snapshot forwarded to the dialog's embedded Sender. */
    commands: CommandInfo[]
}

/**
 * Orchestrate the new-chat dialog handoff:
 *
 * 1. create the (empty) session and select it;
 * 2. when a workspace must be OPENED while none is active on a cold-loaded `/`,
 *    navigate to `/?session=<id>` FIRST — the deep link both bypasses the
 *    `WorkspaceModeSync` `/` → `/workspace` redirect (which would otherwise
 *    fire the moment the workspace cache turns truthy) and survives reloads.
 *    In all other cases no navigation happens (`createSession` already
 *    switched the current session; a `?session=` lock would freeze the
 *    session picker for this tab);
 * 3. apply the workspace selection (open / close / no-op — shared with the
 *    Sender's live dropdown);
 * 4. stage the first message as an auto-submit draft. The chat pane sends it
 *    through the exact same path as a manual submit once its conversation
 *    controller is ready. The draft is only staged after a successful switch,
 *    so a failed open/close never strands a message in the wrong workspace.
 */
export function useAssistantNewChat({ createSession, commands }: UseAssistantNewChatArgs) {
    const [open, setOpen] = useState(false)
    const [submitting, setSubmitting] = useState(false)
    const [selection, setSelection] = useState<WorkspaceSelection>({ kind: "global" })
    const [pendingWorkspaceSwitch, setPendingWorkspaceSwitch] = useState(false)
    const navigate = useNavigate()
    const location = useLocation()
    const setDraft = useWorkspaceHandoffStore((state) => state.setDraft)
    const { applyWorkspace, switching } = useWorkspaceSwitch()

    const workspaceQuery = useQuery({
        queryKey: WORKSPACE_STATE_QUERY_KEY,
        queryFn: workspaceState,
        // Workspace state is fetched over the /v1/workspace HTTP API. The bridge has
        // its own recovery path, so React Query retry would duplicate local probes.
        retry: false,
    })
    const currentWorkspace = workspaceQuery.data?.current ?? null
    const currentRoot = currentWorkspace?.rootPath ?? null

    const openDialog = useCallback(() => {
        // Global (no workspace) is the DEFAULT selection: a new chat starts
        // global unless the user explicitly picks a workspace. The active
        // workspace stays one click away at the top of the list.
        setSelection({ kind: "global" })
        setOpen(true)
    }, [])

    const submit = useCallback(
        async (message: string, options: SenderSubmitOptions) => {
            if (!message.trim()) return
            setSubmitting(true)
            try {
                const session = await createSession({ quiet: false, select: true })
                if (!session) return // the sessions hook already toasted

                const needsSwitch =
                    (selection.kind === "root" && !isSameRoot(currentRoot, selection.rootPath)) ||
                    (selection.kind === "global" && !!currentRoot)

                // Cold-load redirect guard — see the doc comment. We are on the
                // assistant page (`/`) with no active workspace and are about to
                // open one, which would trip WorkspaceModeSync mid-session.
                if (needsSwitch && !currentRoot && selection.kind === "root" && location.pathname === "/") {
                    navigate(`/?session=${session.id}`, { replace: true })
                }

                if (needsSwitch) {
                    setPendingWorkspaceSwitch(true)
                    try {
                        await applyWorkspace(selection, currentRoot)
                    } finally {
                        setPendingWorkspaceSwitch(false)
                    }
                }

                setDraft({
                    autoSubmit: true,
                    prompt: message,
                    sessionId: session.id,
                    files: options.files,
                    effort: options.effort,
                    permissionMode: options.permissionMode,
                    agentType: options.agentType,
                })
                setOpen(false)
            } catch {
                // `applyWorkspace` already toasted; keep the dialog open with the
                // composed message intact so the user can retry.
            } finally {
                setSubmitting(false)
            }
        },
        [applyWorkspace, createSession, currentRoot, location.pathname, navigate, selection, setDraft],
    )

    const dialog: ReactNode = (
        <AssistantNewChatDialog
            open={open}
            onOpenChange={setOpen}
            selection={selection}
            onSelectionChange={setSelection}
            currentWorkspace={currentWorkspace}
            commands={commands}
            onSubmit={submit}
            submitting={submitting || switching}
        />
    )

    return { dialog, openDialog, pendingWorkspaceSwitch, currentWorkspace }
}

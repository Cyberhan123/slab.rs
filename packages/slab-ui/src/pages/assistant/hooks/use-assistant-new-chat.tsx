import { useCallback, useEffect, useState, type ReactNode } from "react"
import { useQuery } from "@tanstack/react-query"
import { useNavigate } from "react-router-dom"

import type { CommandInfo } from "@slab/core/harness"
import { WORKSPACE_STATE_QUERY_KEY, workspaceState } from "@slab/core/workspace/bridge"
import { useWorkspaceHandoffStore } from "@slab/ui/store/useWorkspaceHandoffStore"

import { AssistantNewChatLanding } from "../components/assistant-new-chat-landing"
import type { AssistantConversationItem } from "./use-assistant-sessions"
import type { SenderSubmitOptions } from "../components/sender"
import { isSameRoot, useWorkspaceSwitch, type WorkspaceSelection } from "./use-workspace-switch"

type CreateSessionFn = (options?: { quiet?: boolean; select?: boolean }) => Promise<
    { id: string } | null
>

type UseAssistantNewChatArgs = {
    /** `useAssistantSessions().createSession` — creates and selects a session. */
    createSession: CreateSessionFn
    /** Command registry snapshot forwarded to the landing's embedded Sender. */
    commands: CommandInfo[]
    /** Whether the landing (assistant homepage, no `?session=` deep link) is showing. */
    active: boolean
    /** Recent conversations rendered below the landing composer. */
    conversations: AssistantConversationItem[]
    onSelectConversation: (key: string) => void
    conversationsBusy: boolean
}

/**
 * Orchestrate the new-chat landing — the assistant homepage — and its handoff
 * into the conversation detail page:
 *
 * 1. create the (empty) session and select it;
 * 2. navigate to `/?session=<id>` FIRST — the deep link both turns the page
 *    into the conversation detail and bypasses the `WorkspaceModeSync`
 *    `/` → `/workspace` redirect (which would otherwise fire the moment the
 *    workspace cache turns truthy while a workspace OPENS), and survives
 *    reloads;
 * 3. apply the workspace selection (open / close / no-op — shared with the
 *    detail Sender's live dropdown);
 * 4. stage the first message as an auto-submit draft. The detail chat pane
 *    sends it through the exact same path as a manual submit once its
 *    conversation controller is ready. The draft is only staged after a
 *    successful switch, so a failed open/close never strands a message in the
 *    wrong workspace.
 *
 * While active, the landing also claims any staged workspace handoff draft
 * without a session (e.g. "Explain this code"): the claim effect only mutates
 * the external store, and the prompt seeds the composer through the draft
 * subscription (the Sender's initial value is read on its first render, before
 * the claim flips the store draft to null).
 */
export function useAssistantNewChat({
    createSession,
    commands,
    active,
    conversations,
    onSelectConversation,
    conversationsBusy,
}: UseAssistantNewChatArgs) {
    const [submitting, setSubmitting] = useState(false)
    const [pendingWorkspaceSwitch, setPendingWorkspaceSwitch] = useState(false)
    const navigate = useNavigate()
    const setDraft = useWorkspaceHandoffStore((state) => state.setDraft)
    const handoffDraft = useWorkspaceHandoffStore((state) => state.draft)
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

    // Claim a staged handoff draft that has no session yet (the workspace
    // page's "Explain with assistant" prefill): it used to be consumed by the
    // chat pane, but the homepage has no pane — it prefills the composer
    // instead. External-store mutation only (claim-once), so effect re-runs
    // can't double-claim.
    useEffect(() => {
        if (!active) return
        const draft = useWorkspaceHandoffStore.getState().draft
        if (!draft || draft.sessionId) return
        useWorkspaceHandoffStore.getState().consumeDraft()
    }, [active])

    const submit = useCallback(
        async (message: string, options: SenderSubmitOptions, selection: WorkspaceSelection) => {
            if (!message.trim()) return
            setSubmitting(true)
            try {
                const session = await createSession({ quiet: false, select: true })
                if (!session) return // the sessions hook already toasted

                // Enter the conversation detail BEFORE any workspace switch: the
                // `?session=` deep link pins the page against the WorkspaceModeSync
                // cold-load redirect while the workspace cache turns truthy.
                navigate(`/?session=${encodeURIComponent(session.id)}`)

                const needsSwitch =
                    (selection.kind === "root" && !isSameRoot(currentRoot, selection.rootPath)) ||
                    (selection.kind === "global" && !!currentRoot)

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
            } catch {
                // `applyWorkspace` already toasted. The landing is behind us (we
                // navigated), so the user can head back and retry — the draft was
                // never staged, nothing is stranded.
            } finally {
                setSubmitting(false)
            }
        },
        [applyWorkspace, createSession, currentRoot, navigate, setDraft],
    )

    // Derived (never local state): while a session-less draft sits in the
    // store, its prompt seeds the freshly mounted landing Sender.
    const pendingPrefill =
        handoffDraft && !handoffDraft.sessionId ? handoffDraft.prompt : undefined

    const landing: ReactNode = (
        <AssistantNewChatLanding
            currentWorkspace={currentWorkspace}
            commands={commands}
            onSubmit={submit}
            submitting={submitting || switching}
            initialPrompt={pendingPrefill}
            conversations={conversations}
            onSelectConversation={onSelectConversation}
            conversationsBusy={conversationsBusy}
        />
    )

    return { landing, pendingWorkspaceSwitch, currentWorkspace }
}

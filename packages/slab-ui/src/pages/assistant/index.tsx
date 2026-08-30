"use client"

import { useCallback, useEffect, useRef, useState } from "react"
import { useNavigate, useSearchParams } from "react-router-dom"
import { toast } from "sonner"

import { useTranslation } from "@slab/i18n"

import { AssistantChatPane } from "./components/assistant-chat-pane"
import { AssistantModelSwitchDialog } from "./components/assistant-model-switch-dialog"
import { AssistantSessionSheet } from "./components/assistant-session-sheet"
import { getAssistantErrorDescription } from "./lib/assistant-request-errors"
import { useAssistantConversationList } from "./hooks/use-assistant-conversation-list"
import { useAssistantHeader } from "./hooks/use-assistant-header"
import { useAssistantModel } from "./hooks/use-assistant-model"
import { useAssistantModelStatusLabel } from "./hooks/use-assistant-model-status-label"
import { useAssistantModelSwitch } from "./hooks/use-assistant-model-switch"
import { useAssistantNewChat } from "./hooks/use-assistant-new-chat"
import { useAssistantSessions } from "./hooks/use-assistant-sessions"
import { useHarnessConversation } from "./hooks/use-harness-conversation"
import { useWorkspaceSwitch, type WorkspaceSelection } from "./hooks/use-workspace-switch"
import { useWorkspaceHandoffStore } from "@slab/ui/store/useWorkspaceHandoffStore"
import { WorkspaceSelector } from "@slab/ui/components/workspace-selector"
import { useWorkspaceUiStore } from "@slab/ui/store/useWorkspaceUiStore"

function Assistant() {
    const { t } = useTranslation()
    const navigate = useNavigate()
    const [isSessionSheetOpen, setIsSessionSheetOpen] = useState(false)
    const [isChatBusy, setIsChatBusy] = useState(false)
    const [messageCount, setMessageCount] = useState(0)
    // URL-driven two-view page: `/` is the new-chat landing (the homepage),
    // `/?session=<id>` is the conversation detail. The deep link pins the page
    // to that session, bypassing the shared `zustand:assistant-ui` "current
    // session" (which is global per server and would race across concurrent
    // e2e browsers). Absent the param, the landing shows.
    const [searchParams] = useSearchParams()
    const sessionOverride = searchParams.get("session")
    const trimmedSessionOverride = sessionOverride?.trim() || undefined
    const isDetailView = Boolean(trimmedSessionOverride)

    const {
        conversationList,
        createSession: createEmptySession,
        currentSessionId: curConversation,
        deleteSession: deleteConversationSession,
        isCreatingSession,
        isDeletingSession,
        isSessionMutating,
        isSessionsLoading: sessionsLoading,
        updateSessionLabel,
    } = useAssistantSessions({ lockedSessionId: trimmedSessionOverride })

    const {
        modelOptions,
        selectedModel,
        selectedModelId,
        setSelectedModelId,
        modelLoading,
        isPreparingModel,
        loadedModelStatus,
    } = useAssistantModel()

    const {
        transport,
        restoredMessages,
        restoredThreadId,
        activeConversation,
        restoreVersion,
        isHistoryLoading,
        error: harnessError,
        actionError: harnessActionError,
        approvals,
        approvalStatusByItemId,
        liveOutputByItemId,
        livePatchByItemId,
        modelLoad,
        turnUsage,
        historyCreatedAt,
        commands,
        compactionMarkers,
        isCompacting,
        isForking,
        resolveApproval,
        compactThread,
        forkThread,
        userMessageTurnIndex,
        rollbackFromTurn,
        planMode,
        setPlanMode,
        threadStatus,
        abortReason,
        queuedTexts,
        backgroundTasks,
        sendSteering,
        interrupt,
    } = useHarnessConversation(curConversation, selectedModelId || "slab-llama")

    // Context window for the usage consumption bar: prefer the runtime's
    // post-load value, fall back to the catalog-declared context window.
    const usageContextWindow =
        loadedModelStatus?.context_length ?? selectedModel?.contextWindow ?? null

    const isSessionBootstrapping = (sessionsLoading || isCreatingSession) && conversationList.length === 0
    const isSessionBusy = isChatBusy || isPreparingModel || isHistoryLoading || isSessionMutating

    // Entering the landing resets the departed conversation's busy/message
    // counters: the pane (their only writer) is gone, and a stale message count
    // would misroute the model-switch negotiation on the next new chat.
    useEffect(() => {
        if (!isDetailView) {
            setIsChatBusy(false)
            setMessageCount(0)
        }
    }, [isDetailView])

    const {
        pendingModelSwitchId,
        pendingModelSwitch,
        handleModelPickerChange,
        closePendingModelSwitch,
        handleKeepSessionOnModelSwitch,
        handleCreateSessionOnModelSwitch,
    } = useAssistantModelSwitch({
        modelOptions,
        selectedModelId,
        setSelectedModelId,
        isSessionBusy,
        isSessionBootstrapping,
        curConversation,
        messageCount,
        createSession: createEmptySession,
        isCreatingSession,
    })

    // Session navigation: the detail view is `?session=`-driven, so "switch
    // conversation" is a navigation (the store selection is a no-op under the
    // deep link). Leaving every conversation goes back to the landing.
    const navigateToSession = useCallback(
        (sessionId: string) => {
            navigate(`/?session=${encodeURIComponent(sessionId)}`)
        },
        [navigate],
    )
    const navigateToLanding = useCallback(() => {
        // Already home — skip (a same-location navigate would push a
        // redundant history entry, e.g. deleting the current session from
        // the sheet opened on the landing).
        if (!isDetailView) return
        navigate("/")
    }, [isDetailView, navigate])

    const {
        sortedConversations,
        currentConversationLabel,
        setConversationLabelIfNeeded,
        handleSelectConversation,
        handleDeleteConversation,
    } = useAssistantConversationList({
        conversationList,
        curConversation,
        setCurConversation: navigateToSession,
        deleteSession: deleteConversationSession,
        updateSessionLabel,
        isSessionBusy,
        isSessionBootstrapping,
        setIsSessionSheetOpen,
        onCurrentConversationDeleted: navigateToLanding,
    })

    const { statusLabel: selectedModelStatusLabel } = useAssistantModelStatusLabel({
        curConversation,
        isCreatingSession,
        isDeletingSession,
        isHistoryLoading,
        restoredThreadId,
        isPreparingModel,
        isSessionBootstrapping,
        modelLoading,
        selectedModel,
        loadedModelStatus,
    })

    // New-chat landing (the homepage): pick a workspace (or global), compose
    // the first message, then hand off into the detail page — the created
    // session becomes the deep link and the staged draft auto-sends once the
    // pane is ready.
    const { landing, pendingWorkspaceSwitch, currentWorkspace: activeWorkspace } =
        useAssistantNewChat({
            createSession: createEmptySession,
            commands,
            active: !isDetailView,
            conversations: sortedConversations,
            onSelectConversation: navigateToSession,
            conversationsBusy: isSessionBusy || isSessionBootstrapping,
        })

    // Live workspace selector in the detail Sender toolbar: switching here
    // applies immediately (shared open/close path with the landing's submit).
    // Disabled while a turn runs — a switch would interrupt the running agent
    // threads. Opening a root PINS it against the WorkspaceModeSync redirect,
    // so the switch never bounces the user off the running conversation;
    // switching to 全局 (no workspace) needs no pin — the redirect only fires
    // on open.
    const { applyWorkspace, switching: isWorkspaceSwitching } = useWorkspaceSwitch()
    const setAssistantPinnedWorkspaceRoot = useWorkspaceUiStore(
        (state) => state.setAssistantPinnedWorkspaceRoot,
    )
    const liveWorkspaceSelection = activeWorkspace
        ? { kind: "root" as const, rootPath: activeWorkspace.rootPath, name: activeWorkspace.name }
        : { kind: "global" as const }
    const handleLiveWorkspaceChange = useCallback(
        (selection: WorkspaceSelection) => {
            if (selection.kind === "root") {
                setAssistantPinnedWorkspaceRoot(selection.rootPath)
            }
            void applyWorkspace(selection, activeWorkspace?.rootPath ?? null).catch(() => {})
        },
        [activeWorkspace?.rootPath, applyWorkspace, setAssistantPinnedWorkspaceRoot],
    )

    // Draft handoff into the pane (consumed before sending — see the pane).
    const assistantDraft = useWorkspaceHandoffStore((state) => state.draft)
    const consumeDraft = useWorkspaceHandoffStore((state) => state.consumeDraft)
    const autoSend =
        assistantDraft &&
        (!assistantDraft.sessionId || assistantDraft.sessionId === curConversation) &&
        !pendingWorkspaceSwitch
            ? {
                  text: assistantDraft.prompt,
                  files: assistantDraft.files ?? [],
                  metadata: {
                      effort: assistantDraft.effort,
                      permissionMode: assistantDraft.permissionMode,
                      agentType: assistantDraft.agentType,
                  },
              }
            : null

    const openSessionSheet = useCallback(() => setIsSessionSheetOpen(true), [])

    useAssistantHeader({
        modelOptions,
        selectedModelId,
        modelLoading,
        isSessionBusy,
        isSessionBootstrapping,
        pendingModelSwitchId,
        onModelPickerChange: handleModelPickerChange,
        onOpenSessionSheet: openSessionSheet,
        onNewSession: navigateToLanding,
        showNewSessionControl: isDetailView,
    })

    // Toast a restore failure once per distinct error. `t` is read for the label
    // but intentionally omitted from the deps so a post-mount language change
    // (AppLanguageSync) doesn't re-fire the toast for the same error.
    const lastToastedRestoreErrorRef = useRef<string | null>(null)
    useEffect(() => {
        if (!harnessError) {
            lastToastedRestoreErrorRef.current = null
            return
        }
        if (lastToastedRestoreErrorRef.current === harnessError) return
        lastToastedRestoreErrorRef.current = harnessError
        toast.error(t("pages.assistant.toast.failedToLoadSession"), {
            description: getAssistantErrorDescription(
                new Error(harnessError),
                t("common.toasts.unknownError"),
                // eslint-disable-next-line react-hooks/exhaustive-deps
                t
            ),
        })
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [harnessError])

    // Surface a failed `/compact` or `/fork` action (distinct from a restore
    // failure) so a backend refusal doesn't look like "nothing happened".
    const lastToastedActionRef = useRef<{ kind: string; message: string } | null>(null)
    useEffect(() => {
        if (!harnessActionError) {
            lastToastedActionRef.current = null
            return
        }
        const last = lastToastedActionRef.current
        if (last?.kind === harnessActionError.kind && last.message === harnessActionError.message) {
            return
        }
        lastToastedActionRef.current = {
            kind: harnessActionError.kind,
            message: harnessActionError.message,
        }
        toast.error(
            t(
                harnessActionError.kind === "compact"
                    ? "pages.assistant.toast.compactFailed"
                    : "pages.assistant.toast.forkFailed",
            ),
            { description: harnessActionError.message },
        )
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [harnessActionError])

    const handleBeforeSubmit = useCallback(
        async (value: string) => {
            if (!curConversation || isSessionBusy || isSessionBootstrapping) {
                toast.info(t("pages.assistant.toast.sessionSyncing"))
                throw new Error("Assistant session is not ready.")
            }

            // NOTE: model loading is now server-driven inside `turn/start`
            // (streaming `model/load/*` notifications rendered by the in-stream
            // model-load Marker), so there is no HTTP pre-flight here.
            void setConversationLabelIfNeeded(curConversation, value)
        },
        [
            curConversation,
            isSessionBootstrapping,
            isSessionBusy,
            setConversationLabelIfNeeded,
            t,
        ]
    )

    return (
        <>
            {isDetailView ? (
                <AssistantChatPane
                    key={`${curConversation ?? "none"}:${restoreVersion}`}
                    disabled={isSessionBootstrapping || isHistoryLoading || isSessionMutating || !curConversation}
                    initialMessages={restoredMessages}
                    isHistoryLoading={isHistoryLoading}
                    modelStatusLabel={selectedModelStatusLabel}
                    onBeforeSubmit={handleBeforeSubmit}
                    onBusyChange={setIsChatBusy}
                    onMessageCountChange={setMessageCount}
                    transport={transport}
                    approvals={approvals}
                    approvalStatusByItemId={approvalStatusByItemId}
                    liveOutputByItemId={liveOutputByItemId}
                    livePatchByItemId={livePatchByItemId}
                    modelLoad={modelLoad}
                    turnUsage={turnUsage}
                    contextWindow={usageContextWindow}
                    resolveApproval={resolveApproval}
                    onCompact={() => compactThread()}
                    onFork={() => forkThread()}
                    historyCreatedAt={historyCreatedAt}
                    commands={commands}
                    compactionMarkers={compactionMarkers}
                    isCompacting={isCompacting}
                    isForking={isForking}
                    userMessageTurnIndex={userMessageTurnIndex}
                    onRollbackFromTurn={rollbackFromTurn}
                    planMode={planMode}
                    onPlanModeChange={setPlanMode}
                    threadStatus={threadStatus}
                    abortReason={abortReason}
                    queuedTexts={queuedTexts}
                    backgroundTasks={backgroundTasks}
                    onSteerSubmit={async (text, options) => {
                        const result = (await sendSteering(
                            {
                                id: `steer-${Date.now()}`,
                                role: "user",
                                parts: [{ type: "text", text }],
                            },
                            options,
                        )) as { queued?: boolean } | undefined
                        // Lost the idle-window race: the message started a NEW run
                        // whose live stream this pane isn't subscribed to. The
                        // controller refreshes on its terminal event; surface the
                        // fallback so the resync doesn't look like a silent stall.
                        if (!result || result.queued !== true) {
                            toast.info(t("pages.assistant.toast.steeringResync"))
                        }
                    }}
                    onInterrupt={() => {
                        void interrupt().catch(() => {})
                    }}
                    onStartNewChat={navigateToLanding}
                    autoSend={autoSend}
                    onAutoSendConsumed={consumeDraft}
                    workspaceSlot={
                        <WorkspaceSelector
                            value={liveWorkspaceSelection}
                            onValueChange={handleLiveWorkspaceChange}
                            currentWorkspace={activeWorkspace}
                            busy={isWorkspaceSwitching || isChatBusy}
                        />
                    }
                />
            ) : (
                landing
            )}

            <AssistantSessionSheet
                open={isSessionSheetOpen}
                onOpenChange={setIsSessionSheetOpen}
                conversations={sortedConversations}
                currentConversation={curConversation}
                activeConversation={activeConversation}
                busy={isSessionBusy || isSessionBootstrapping}
                onSelect={handleSelectConversation}
                onDelete={handleDeleteConversation}
            />

            <AssistantModelSwitchDialog
                conversationLabel={currentConversationLabel}
                isCreatingSession={isCreatingSession}
                messageCount={messageCount}
                onCreateSession={() => void handleCreateSessionOnModelSwitch()}
                onKeepSession={handleKeepSessionOnModelSwitch}
                onOpenChange={(open) => {
                    if (!open) {
                        closePendingModelSwitch()
                    }
                }}
                pendingModelId={pendingModelSwitchId}
                pendingModelLabel={pendingModelSwitch?.label}
                selectedModelLabel={selectedModel?.label}
            />
        </>
    )
}

export default Assistant
